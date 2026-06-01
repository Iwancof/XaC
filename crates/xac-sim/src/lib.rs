use anyhow::Result;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
#[cfg(test)]
use xac_core::EnemyKind;
use xac_core::{
    BehaviorId, Block, BlockKind, DeliveryJob, Direction, Drone, Enemy, EntityId, ItemFlowEvent,
    ItemKind, LogEntry, LogLevel, Network, Pos, Tile,
};
use xac_wasm::{BehaviorRuntime, CompiledBehavior};

mod behavior;
mod behavior_api;
mod behavior_host;
mod behavior_runner;
mod block_defs;
mod combat;
mod construction;
mod cpu;
mod drone;
mod drone_port;
mod geometry;
mod lifecycle;
mod logistics;
mod network;
mod production;
mod recipes;
mod save;
mod snapshot;
mod user_config;
mod wave;

use behavior::{load_behaviors, BehaviorPackage};
use block_defs::{build_tiles, make_block};
use geometry::footprint_positions;

pub const MAP_WIDTH: i32 = 64;
pub const MAP_HEIGHT: i32 = 64;
pub const TICKS_PER_SECOND: u32 = 20;

pub struct Simulation {
    tick: u64,
    running: bool,
    width: i32,
    height: i32,
    tiles: Vec<Tile>,
    blocks: BTreeMap<EntityId, Block>,
    enemies: BTreeMap<EntityId, Enemy>,
    drones: BTreeMap<EntityId, Drone>,
    networks: BTreeMap<u32, Network>,
    behaviors: BTreeMap<BehaviorId, BehaviorPackage>,
    compiled_behaviors: BTreeMap<BehaviorId, CompiledBehavior>,
    fuel_banks: BTreeMap<EntityId, f32>,
    pending_jobs: Vec<DeliveryJob>,
    item_flows: VecDeque<ItemFlowEvent>,
    logs: VecDeque<LogEntry>,
    selected_id: Option<EntityId>,
    next_id: u64,
    next_flow_id: u64,
    runtime: BehaviorRuntime,
    config_root: PathBuf,
}

impl Simulation {
    pub fn new(config_root: impl AsRef<Path>) -> Result<Self> {
        user_config::ensure_user_config(config_root.as_ref())?;
        let mut sim = Self {
            tick: 0,
            running: false,
            width: MAP_WIDTH,
            height: MAP_HEIGHT,
            tiles: build_tiles(),
            blocks: BTreeMap::new(),
            enemies: BTreeMap::new(),
            drones: BTreeMap::new(),
            networks: BTreeMap::new(),
            behaviors: load_behaviors(config_root.as_ref())?,
            compiled_behaviors: BTreeMap::new(),
            fuel_banks: BTreeMap::new(),
            pending_jobs: Vec::new(),
            item_flows: VecDeque::new(),
            logs: VecDeque::new(),
            selected_id: None,
            next_id: 1,
            next_flow_id: 1,
            runtime: BehaviorRuntime::new()?,
            config_root: config_root.as_ref().to_path_buf(),
        };
        sim.seed_world();
        sim.reserve_next_id_from_existing();
        sim.recompute_networks();
        Ok(sim)
    }

    fn seed_world(&mut self) {
        let core_pos = Pos { x: 30, y: 30 };
        let core_id = self.make_id("core");
        let mut core = make_block(core_id.clone(), BlockKind::Core, core_pos, Direction::East);
        core.inventory.add(ItemKind::Ore, 40);
        core.inventory.add(ItemKind::Plate, 20);
        core.inventory.add(ItemKind::Ammo, 60);
        core.status = "core online".to_string();
        self.set_tile_footprint(BlockKind::Core, core_pos, Some(core_id.clone()));
        self.blocks.insert(core_id.clone(), core);
        self.selected_id = Some(core_id);
        self.log(
            LogLevel::Info,
            "system",
            "XaC MVP world initialized".to_string(),
        );
    }

    pub(crate) fn make_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}_{}", self.next_id);
        self.next_id += 1;
        id
    }

    fn reserve_next_id_from_existing(&mut self) {
        let max_existing = self
            .blocks
            .keys()
            .chain(self.enemies.keys())
            .chain(self.drones.keys())
            .chain(self.behaviors.keys())
            .filter_map(|id| id.rsplit_once('_'))
            .filter_map(|(_, suffix)| suffix.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(max_existing + 1);
    }

    fn in_bounds(&self, pos: Pos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height
    }

    fn tile_index(&self, pos: Pos) -> Option<usize> {
        if self.in_bounds(pos) {
            Some((pos.y * self.width + pos.x) as usize)
        } else {
            None
        }
    }

    fn tile_at(&self, pos: Pos) -> Option<&Tile> {
        self.tile_index(pos).and_then(|idx| self.tiles.get(idx))
    }

    fn block_id_at(&self, pos: Pos) -> Option<EntityId> {
        self.tile_at(pos).and_then(|t| t.block_id.clone())
    }

    fn set_tile_footprint(&mut self, kind: BlockKind, pos: Pos, block_id: Option<EntityId>) {
        for tile in footprint_positions(kind, pos) {
            if let Some(idx) = self.tile_index(tile) {
                self.tiles[idx].block_id = block_id.clone();
            }
        }
    }

    fn remove_block_references(&mut self, block_id: &str) {
        self.fuel_banks.remove(block_id);
        self.pending_jobs
            .retain(|job| job.pickup != block_id && job.dropoff != block_id);
        let removed_home_drones: Vec<_> = self
            .drones
            .values()
            .filter(|drone| drone.home_port == block_id)
            .map(|drone| drone.id.clone())
            .collect();
        for drone_id in removed_home_drones {
            self.drones.remove(&drone_id);
            self.fuel_banks.remove(&drone_id);
        }
        for drone in self.drones.values_mut() {
            if drone
                .job
                .as_ref()
                .map(|job| job.pickup == block_id || job.dropoff == block_id)
                .unwrap_or(false)
            {
                drone.job = None;
                drone.state = xac_core::DroneState::Returning;
            }
        }
    }

    fn log(&mut self, level: LogLevel, source: impl Into<String>, message: String) {
        self.logs.push_back(LogEntry {
            tick: self.tick,
            level,
            source: source.into(),
            message,
        });
        while self.logs.len() > 160 {
            self.logs.pop_front();
        }
    }
}

#[cfg(test)]
mod tests;
