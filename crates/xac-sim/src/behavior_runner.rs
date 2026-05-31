use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use xac_core::{BehaviorKind, Direction, ItemKind, LogLevel};
use xac_wasm::{BehaviorHostInput, BehaviorIntent, CompiledBehavior, NetStoreWrite};

use crate::{Simulation, TICKS_PER_SECOND};

const MIN_BEHAVIOR_INVOCATION_FUEL: u64 = 40;
const BEHAVIOR_FUEL_BANK_SECONDS: f32 = 8.0;

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
            let fuel = self.grant_behavior_fuel(&id, cpu_rate);
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
                    self.spend_behavior_fuel(&id, eval.fuel_spent);
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
                        package.wasm_hash = Some(eval.wasm_hash);
                    }
                    self.apply_net_writes(&id, eval.net_writes);
                    self.apply_behavior_intent(&id, eval.intent);
                }
                Err(error) => {
                    self.spend_behavior_fuel(&id, fuel);
                    if let Some(block) = self.blocks.get_mut(&id) {
                        block.status = "runtime error".to_string();
                    }
                    self.log(LogLevel::Error, id, error.to_string());
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
        BehaviorHostInput {
            output_blocked: self.output_blocked(block_id),
            can_produce: self.can_produce(block_id),
            assembler_can_produce: [
                self.can_progress_recipe(block_id, ItemKind::Plate.as_str()),
                self.can_progress_recipe(block_id, ItemKind::Ammo.as_str()),
            ],
            assembler_input_counts: block_inventory_counts.clone(),
            assembler_output_counts: block_inventory_counts,
            ammo_count: block.inventory.count(&ItemKind::Ammo) as i32,
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
            drone_port_stock_counts: self.network_stock_counts(block_id),
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

    fn apply_net_writes(&mut self, block_id: &str, writes: Vec<NetStoreWrite>) {
        if writes.is_empty() {
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
        for write in writes {
            network
                .store
                .insert(write.key.to_string(), serde_json::Value::from(write.value));
        }
    }

    fn grant_behavior_fuel(&mut self, block_id: &str, cpu_rate: f32) -> u64 {
        let bank = self.fuel_banks.entry(block_id.to_string()).or_insert(0.0);
        let max_bank = (cpu_rate.max(1.0) * BEHAVIOR_FUEL_BANK_SECONDS)
            .max(MIN_BEHAVIOR_INVOCATION_FUEL as f32);
        *bank = (*bank + cpu_rate / TICKS_PER_SECOND as f32).min(max_bank);
        let available = bank.floor() as u64;
        if available >= MIN_BEHAVIOR_INVOCATION_FUEL {
            available
        } else {
            0
        }
    }

    fn spend_behavior_fuel(&mut self, block_id: &str, fuel_spent: u64) {
        if let Some(bank) = self.fuel_banks.get_mut(block_id) {
            *bank = (*bank - fuel_spent as f32).max(0.0);
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
        let wasm_hash = compiled.wasm_hash().to_string();
        if let Some(package) = self.behaviors.get_mut(behavior_id) {
            package.wasm_hash = Some(wasm_hash);
            if package.summary.build_status != "built" {
                package.summary.build_status = "compiled".to_string();
            }
        }
        self.compiled_behaviors
            .insert(behavior_id.to_string(), compiled.clone());
        Ok(compiled)
    }

    fn apply_behavior_intent(&mut self, block_id: &str, intent: BehaviorIntent) {
        match intent {
            BehaviorIntent::Noop => {}
            BehaviorIntent::DrillDefault => self.run_drill(block_id),
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
            BehaviorIntent::Assembler { recipe } => {
                let recipe_id = recipe.as_str().to_string();
                if let Some(block) = self.blocks.get_mut(block_id) {
                    block.recipe = Some(recipe_id.clone());
                    block.status = format!("recipe: {recipe_id} priority");
                }
                self.run_assembler(block_id);
            }
            BehaviorIntent::Turret { priority } => self.run_turret_once(block_id, &priority),
            BehaviorIntent::DronePort { commands } => {
                self.apply_drone_port_commands(block_id, commands);
            }
            BehaviorIntent::CarrierDrone { .. } => {}
        }
    }

    fn network_stock_counts(&self, block_id: &str) -> BTreeMap<ItemKind, i32> {
        let mut counts = BTreeMap::new();
        let Some(block) = self.blocks.get(block_id) else {
            return counts;
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
        }
        counts
    }
}
