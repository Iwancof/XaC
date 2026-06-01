use std::collections::{BTreeMap, BTreeSet, VecDeque};

use xac_core::{BlockKind, Direction, DroneState, Network};

use crate::geometry::footprint_positions;
use crate::Simulation;

impl Simulation {
    pub(crate) fn recompute_networks(&mut self) {
        for block in self.blocks.values_mut() {
            block.network_id = None;
            block.effective_cpu_rate = block.kind.local_cpu_rate();
            block.active = block.kind.is_programmable();
        }
        let previous_networks = std::mem::take(&mut self.networks);

        let connector_positions: BTreeSet<_> = self
            .blocks
            .values()
            .filter(|b| b.kind.is_network_connector())
            .flat_map(|b| footprint_positions(b.kind, b.pos))
            .collect();
        let mut seen = BTreeSet::new();
        let mut next_network = 1_u32;

        for start in connector_positions.iter().copied() {
            if seen.contains(&start) {
                continue;
            }
            let component = connected_component(start, &connector_positions, &mut seen);
            let block_ids = self.network_block_ids(&component);
            let store = inherit_store(&block_ids, &previous_networks);
            let cpu_pool = block_ids
                .iter()
                .filter_map(|id| self.blocks.get(id))
                .map(|block| block.kind.network_cpu_output())
                .sum::<f32>();
            let active_devices = block_ids
                .iter()
                .filter_map(|id| self.blocks.get(id))
                .filter(|b| b.kind.is_programmable())
                .count() as u32
                + self.docked_drone_count_in_network(&block_ids);
            let effective_per_device = if active_devices > 0 {
                cpu_pool / active_devices as f32
            } else {
                0.0
            };
            for id in &block_ids {
                if let Some(block) = self.blocks.get_mut(id) {
                    block.network_id = Some(next_network);
                    if block.kind.is_programmable() {
                        block.effective_cpu_rate =
                            block.kind.local_cpu_rate() + effective_per_device;
                    }
                }
            }
            let read_only_cache = !block_ids.iter().any(|id| {
                self.blocks
                    .get(id)
                    .map(|block| block.kind == BlockKind::Core)
                    .unwrap_or(false)
            });
            self.networks.insert(
                next_network,
                Network {
                    id: next_network,
                    cpu_pool,
                    active_devices,
                    effective_per_device,
                    block_ids,
                    store,
                    read_only_cache,
                },
            );
            next_network += 1;
        }
    }

    fn network_block_ids(&self, component: &[xac_core::Pos]) -> Vec<String> {
        let mut block_ids = BTreeSet::new();
        for pos in component {
            if let Some(id) = self.block_id_at(*pos) {
                block_ids.insert(id);
            }
            for dir in Direction::all() {
                if let Some(id) = self.block_id_at(pos.step(dir)) {
                    if self
                        .blocks
                        .get(&id)
                        .map(|block| block.kind.is_network_node())
                        .unwrap_or(false)
                    {
                        block_ids.insert(id);
                    }
                }
            }
        }
        block_ids.into_iter().collect()
    }

    fn docked_drone_count_in_network(&self, block_ids: &[String]) -> u32 {
        self.drones
            .values()
            .filter(|drone| {
                drone.state == DroneState::Docked
                    && block_ids
                        .iter()
                        .any(|block_id| block_id == &drone.home_port)
            })
            .count() as u32
    }
}

fn inherit_store(
    block_ids: &[String],
    previous_networks: &BTreeMap<u32, Network>,
) -> BTreeMap<String, serde_json::Value> {
    let mut overlapping: Vec<_> = previous_networks
        .values()
        .filter_map(|network| {
            let overlap = block_ids
                .iter()
                .filter(|id| network.block_ids.contains(id))
                .count();
            (overlap > 0).then_some((overlap, &network.store))
        })
        .collect();
    overlapping.sort_by_key(|(overlap, _)| *overlap);

    let mut merged = BTreeMap::new();
    for (_, store) in overlapping {
        merged.extend(store.clone());
    }
    merged
}

fn connected_component(
    start: xac_core::Pos,
    connector_positions: &BTreeSet<xac_core::Pos>,
    seen: &mut BTreeSet<xac_core::Pos>,
) -> Vec<xac_core::Pos> {
    let mut queue = VecDeque::from([start]);
    let mut component = Vec::new();
    seen.insert(start);
    while let Some(pos) = queue.pop_front() {
        component.push(pos);
        for dir in Direction::all() {
            let next = pos.step(dir);
            if connector_positions.contains(&next) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    component
}
