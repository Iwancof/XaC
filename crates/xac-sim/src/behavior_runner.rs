use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use xac_core::{BehaviorKind, Direction, EnemyKind, ItemKind, LogLevel, TerrainKind};
use xac_wasm::{
    AssemblerCommand, BehaviorHostInput, BehaviorIntent, BehaviorLog, CompiledBehavior,
    DrillCommand, NetStoreOp,
};

use crate::behavior::{persist_compiled_behavior_cache, persist_project_behavior_index};
use crate::cpu::FuelPolicy;
use crate::Simulation;

const BLOCK_BEHAVIOR_FUEL_POLICY: FuelPolicy = FuelPolicy {
    min_invocation_fuel: 40,
    max_bank_seconds: 8.0,
};

impl Simulation {
    pub(crate) fn run_programmable_behaviors(&mut self) {
        let ids: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.kind.is_programmable() && b.active)
            .map(|b| b.id.clone())
            .collect();

        for id in ids {
            let (kind, behavior_ref, cpu_rate) = match self.blocks.get(&id) {
                Some(block) => (
                    BehaviorKind::from_block_kind(block.kind)
                        .expect("programmable blocks have behavior kinds"),
                    block.behavior_ref.clone(),
                    block.effective_cpu_rate,
                ),
                None => continue,
            };
            let Some(behavior_ref) = behavior_ref else {
                continue;
            };
            let fuel = self.grant_fuel_bank(&id, cpu_rate, BLOCK_BEHAVIOR_FUEL_POLICY);
            if fuel == 0 {
                continue;
            }
            let compiled = match self.compiled_behavior(&behavior_ref, kind) {
                Ok(compiled) => compiled,
                Err(error) => {
                    if let Some(block) = self.blocks.get_mut(&id) {
                        block.status = "runtime error".to_string();
                    }
                    self.log(LogLevel::Error, id, error.to_string());
                    continue;
                }
            };
            let host_input = self.behavior_host_input(&id);

            match self.runtime.evaluate_compiled(&compiled, fuel, host_input) {
                Ok(eval) => {
                    let runtime_error = eval.runtime_error.clone();
                    let wasm_hash = eval.wasm_hash.clone();
                    self.record_block_behavior_runtime(&id, fuel, &eval);
                    self.spend_fuel_bank(&id, eval.fuel_spent);
                    if let Some(error) = runtime_error {
                        if let Some(package) = self.behaviors.get_mut(&behavior_ref) {
                            package.wasm_hash = Some(wasm_hash);
                        }
                        if let Some(block) = self.blocks.get_mut(&id) {
                            block.status = "runtime error".to_string();
                        }
                        self.log(LogLevel::Error, id, error);
                        continue;
                    }
                    if eval.over_budget {
                        if let Some(block) = self.blocks.get_mut(&id) {
                            block.status = "over_budget".to_string();
                        }
                        self.log(
                            LogLevel::Warn,
                            id.clone(),
                            format!("over_budget with {fuel} fuel"),
                        );
                        continue;
                    }
                    if let Some(package) = self.behaviors.get_mut(&behavior_ref) {
                        package.wasm_hash = Some(wasm_hash);
                    }
                    self.apply_net_ops(&id, eval.net_ops);
                    self.apply_behavior_logs(&id, eval.logs);
                    self.apply_behavior_intent(&id, eval.intent);
                }
                Err(error) => {
                    let message = error.to_string();
                    self.spend_fuel_bank(&id, fuel);
                    self.record_block_behavior_runtime_error(
                        &id,
                        fuel,
                        fuel,
                        0,
                        Some(compiled.wasm_hash().to_string()),
                        message.clone(),
                    );
                    if let Some(block) = self.blocks.get_mut(&id) {
                        block.status = "runtime error".to_string();
                    }
                    self.log(LogLevel::Error, id, message);
                }
            }
        }
    }

    fn behavior_host_input(&self, block_id: &str) -> BehaviorHostInput {
        let Some(block) = self.blocks.get(block_id) else {
            return BehaviorHostInput::default();
        };
        let block_inventory_counts: BTreeMap<ItemKind, i32> = block
            .inventory
            .items
            .iter()
            .map(|(item, amount)| (item.clone(), i32::try_from(*amount).unwrap_or(i32::MAX)))
            .collect();
        let (network_stock_counts, network_stock_capacity, network_stock_space) =
            self.network_stock_profile(block_id);
        let turret_scan = if block.kind == xac_core::BlockKind::Turret {
            self.turret_visible_enemy_scan(block_id)
        } else {
            Vec::new()
        };
        BehaviorHostInput {
            inventory_counts: block_inventory_counts.clone(),
            inventory_free: i32::try_from(
                block
                    .inventory
                    .capacity
                    .saturating_sub(block.inventory.total()),
            )
            .unwrap_or(i32::MAX),
            output_blocked: self.output_blocked(block_id),
            drill_ore_kind: self.drill_ore_kind(block_id),
            drill_can_mine: self.drill_can_mine(block_id),
            drill_output_available: ItemKind::all()
                .into_iter()
                .map(|item| {
                    let available = self.output_item_available(block_id, &item, block.dir);
                    (item, available)
                })
                .collect(),
            can_produce: self.can_produce(block_id),
            assembler_can_produce: [
                self.can_progress_recipe(block_id, ItemKind::Plate.as_str()),
                self.can_progress_recipe(block_id, ItemKind::Ammo.as_str()),
            ],
            assembler_current_recipe: block.recipe.as_deref().and_then(ItemKind::from_id),
            assembler_input_counts: block_inventory_counts.clone(),
            assembler_output_counts: block_inventory_counts,
            ammo_count: block.inventory.count(&ItemKind::Ammo) as i32,
            turret_visible_enemy_count: i32::try_from(turret_scan.len()).unwrap_or(i32::MAX),
            turret_visible_enemy_kinds: turret_scan
                .iter()
                .map(|(kind, _, _)| *kind)
                .collect::<Vec<EnemyKind>>(),
            turret_visible_enemy_hp: turret_scan.iter().map(|(_, hp, _)| *hp).collect(),
            turret_visible_enemy_distance: turret_scan
                .iter()
                .map(|(_, _, distance)| *distance)
                .collect(),
            router_output_available: Direction::all()
                .map(|dir| self.output_available(block_id, dir)),
            router_item_output_available: ItemKind::all()
                .into_iter()
                .map(|item| {
                    let by_dir = Direction::all()
                        .map(|dir| self.output_item_available(block_id, &item, dir));
                    (item, by_dir)
                })
                .collect(),
            network_stock_counts: network_stock_counts.clone(),
            network_stock_capacity,
            network_stock_space,
            drone_port_stock_counts: network_stock_counts,
            drone_port_docked_drone_count: self.docked_drone_count_at_port(block_id),
            drone_port_pending_job_count: i32::try_from(self.pending_jobs.len())
                .unwrap_or(i32::MAX),
            net_i32: self.network_i32_values(block.network_id),
            net_writable: block
                .network_id
                .and_then(|network_id| self.networks.get(&network_id))
                .map(|network| !network.read_only_cache)
                .unwrap_or(false),
            ..Default::default()
        }
    }

    fn network_i32_values(&self, network_id: Option<u32>) -> BTreeMap<i32, i32> {
        let Some(network_id) = network_id else {
            return BTreeMap::new();
        };
        let Some(network) = self.networks.get(&network_id) else {
            return BTreeMap::new();
        };
        network
            .store
            .iter()
            .filter_map(|(key, value)| {
                Some((
                    key.parse().ok()?,
                    value.as_i64().and_then(|v| i32::try_from(v).ok())?,
                ))
            })
            .collect()
    }

    fn apply_net_ops(&mut self, block_id: &str, ops: Vec<NetStoreOp>) {
        if ops.is_empty() {
            return;
        }
        let Some(network_id) = self.blocks.get(block_id).and_then(|block| block.network_id) else {
            return;
        };
        let Some(network) = self.networks.get_mut(&network_id) else {
            return;
        };
        if network.read_only_cache {
            return;
        }
        for op in ops {
            match op {
                NetStoreOp::Set(write) => {
                    network
                        .store
                        .insert(write.key.to_string(), serde_json::Value::from(write.value));
                }
                NetStoreOp::Delete(delete) => {
                    network.store.remove(&delete.key.to_string());
                }
            }
        }
    }

    pub(crate) fn apply_behavior_logs(&mut self, block_id: &str, logs: Vec<BehaviorLog>) {
        for entry in logs {
            self.log(LogLevel::Info, block_id.to_string(), entry.message);
        }
    }

    pub(crate) fn compiled_behavior(
        &mut self,
        behavior_id: &str,
        expected_kind: BehaviorKind,
    ) -> Result<CompiledBehavior> {
        if let Some(compiled) = self.compiled_behaviors.get(behavior_id) {
            return Ok(compiled.clone());
        }

        let (kind, source) = {
            let package = self
                .behaviors
                .get(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            (package.summary.base_kind, package.source.clone())
        };
        if kind != expected_kind {
            return Err(anyhow!(
                "behavior {behavior_id} targets {kind:?}, but block is {expected_kind:?}"
            ));
        }

        let compiled = self.runtime.compile_wat(kind, &source)?;
        {
            let package = self
                .behaviors
                .get(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            persist_compiled_behavior_cache(&self.config_root, package, &compiled)?;
        }
        let wasm_hash = compiled.wasm_hash().to_string();
        let mut persist_index = false;
        if let Some(package) = self.behaviors.get_mut(behavior_id) {
            package.wasm_hash = Some(wasm_hash);
            if package.summary.build_status != "built" {
                package.summary.build_status = "compiled".to_string();
            }
            persist_index = !package.summary.builtin;
        }
        if persist_index {
            persist_project_behavior_index(&self.config_root, &self.behaviors)?;
        }
        self.compiled_behaviors
            .insert(behavior_id.to_string(), compiled.clone());
        Ok(compiled)
    }

    fn apply_behavior_intent(&mut self, block_id: &str, intent: BehaviorIntent) {
        match intent {
            BehaviorIntent::Noop => {}
            BehaviorIntent::Drill { commands } => {
                for command in commands {
                    match command {
                        DrillCommand::Mine => self.run_drill(block_id),
                        DrillCommand::Output { item } => {
                            let Some(dir) = self.blocks.get(block_id).map(|block| block.dir) else {
                                continue;
                            };
                            self.transfer_item_from(block_id, dir, &item, 1);
                        }
                    }
                }
            }
            BehaviorIntent::Router { item, preferred } => {
                let dirs = if preferred.is_empty() {
                    Direction::all().to_vec()
                } else {
                    preferred
                };
                for dir in dirs {
                    let moved = if let Some(item) = item.as_ref() {
                        self.transfer_item_from(block_id, dir, item, 1)
                    } else {
                        self.transfer_from(block_id, dir, 1)
                    };
                    if moved {
                        break;
                    }
                }
            }
            BehaviorIntent::Assembler { commands } => {
                for command in commands {
                    match command {
                        AssemblerCommand::SetRecipe { recipe } => {
                            let recipe_id = recipe.as_str().to_string();
                            if let Some(block) = self.blocks.get_mut(block_id) {
                                block.recipe = Some(recipe_id.clone());
                                block.status = format!("recipe: {recipe_id} priority");
                            }
                        }
                        AssemblerCommand::Produce { recipe } => {
                            let recipe_id = recipe.as_str().to_string();
                            if let Some(block) = self.blocks.get_mut(block_id) {
                                block.recipe = Some(recipe_id);
                            }
                            self.run_assembler(block_id);
                        }
                    }
                }
            }
            BehaviorIntent::Turret { priority } => self.run_turret_once(block_id, &priority),
            BehaviorIntent::TurretScanIndex { index } => {
                self.run_turret_scan_index(block_id, index);
            }
            BehaviorIntent::DronePort { commands } => {
                self.apply_drone_port_commands(block_id, commands);
            }
            BehaviorIntent::CarrierDrone { .. } => {}
        }
    }

    fn network_stock_profile(
        &self,
        block_id: &str,
    ) -> (
        BTreeMap<ItemKind, i32>,
        BTreeMap<ItemKind, i32>,
        BTreeMap<ItemKind, i32>,
    ) {
        let mut counts = BTreeMap::new();
        let mut capacity = BTreeMap::new();
        let mut space = BTreeMap::new();
        let Some(block) = self.blocks.get(block_id) else {
            return (counts, capacity, space);
        };
        let ids: Vec<&str> = block
            .network_id
            .and_then(|network_id| self.networks.get(&network_id))
            .map(|network| network.block_ids.iter().map(String::as_str).collect())
            .unwrap_or_else(|| vec![block_id]);
        for id in ids {
            let Some(block) = self.blocks.get(id) else {
                continue;
            };
            for (item, amount) in &block.inventory.items {
                let total = counts.entry(item.clone()).or_insert(0_i32);
                *total = total.saturating_add(i32::try_from(*amount).unwrap_or(i32::MAX));
            }
            for item in ItemKind::all() {
                if !block.kind.can_accept_item(&item) {
                    continue;
                }
                let total_capacity = capacity.entry(item.clone()).or_insert(0_i32);
                *total_capacity = total_capacity
                    .saturating_add(i32::try_from(block.inventory.capacity).unwrap_or(i32::MAX));
                let total_space = space.entry(item).or_insert(0_i32);
                let free = block
                    .inventory
                    .capacity
                    .saturating_sub(block.inventory.total());
                *total_space = total_space.saturating_add(i32::try_from(free).unwrap_or(i32::MAX));
            }
        }
        (counts, capacity, space)
    }

    fn drill_ore_kind(&self, block_id: &str) -> Option<ItemKind> {
        let block = self.blocks.get(block_id)?;
        self.tile_at(block.pos)
            .is_some_and(|tile| tile.terrain == TerrainKind::OrePatch)
            .then_some(ItemKind::Ore)
    }
}
