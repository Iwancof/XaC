use std::collections::BTreeMap;

use xac_core::{Direction, EnemyKind, ItemKind, LogLevel, TerrainKind};
use xac_wasm::{BehaviorHostInput, BehaviorLog, NetStoreOp};

use crate::Simulation;

impl Simulation {
    pub(crate) fn behavior_host_input(&self, block_id: &str) -> BehaviorHostInput {
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

    pub(crate) fn apply_net_ops(&mut self, block_id: &str, ops: Vec<NetStoreOp>) {
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
