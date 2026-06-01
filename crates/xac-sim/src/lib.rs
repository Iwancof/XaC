use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use xac_core::{
    BehaviorId, Block, BlockKind, DeliveryJob, Direction, Drone, Enemy, EnemyKind, EntityId,
    GameSnapshot, GameStatus, ItemKind, LogEntry, LogLevel, Network, Pos, Tile,
};
use xac_wasm::{BehaviorRuntime, CompiledBehavior};

mod behavior;
mod behavior_api;
mod behavior_runner;
mod block_defs;
mod combat;
mod drone;
mod geometry;
mod logistics;
mod network;
mod production;
mod recipes;
mod wave;

use behavior::{load_behaviors, BehaviorPackage};
use block_defs::{build_tiles, default_behavior_for, kind_name, make_block};
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
    logs: VecDeque<LogEntry>,
    selected_id: Option<EntityId>,
    next_id: u64,
    runtime: BehaviorRuntime,
    config_root: PathBuf,
}

impl Simulation {
    pub fn new(config_root: impl AsRef<Path>) -> Result<Self> {
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
            logs: VecDeque::new(),
            selected_id: None,
            next_id: 1,
            runtime: BehaviorRuntime::new()?,
            config_root: config_root.as_ref().to_path_buf(),
        };
        sim.seed_world();
        sim.reserve_next_id_from_existing();
        sim.recompute_networks();
        Ok(sim)
    }

    pub fn set_running(&mut self, running: bool) -> GameSnapshot {
        self.running = running && !self.core_defeated();
        self.snapshot()
    }

    pub fn step_ticks(&mut self, count: u32) -> GameSnapshot {
        for _ in 0..count.min(500) {
            self.tick_once();
        }
        self.snapshot()
    }

    pub fn update_if_running(&mut self, max_ticks: u32) -> GameSnapshot {
        if self.running {
            self.step_ticks(max_ticks)
        } else {
            self.snapshot()
        }
    }

    pub fn place_block(
        &mut self,
        kind: BlockKind,
        pos: Pos,
        dir: Direction,
    ) -> Result<GameSnapshot> {
        if kind == BlockKind::Core {
            return Err(anyhow!(
                "core is the initial 4x4 objective and cannot be placed"
            ));
        }
        let footprint = footprint_positions(kind, pos);
        if footprint.iter().any(|tile| !self.in_bounds(*tile)) {
            return Err(anyhow!("position is outside the map"));
        }
        for tile in &footprint {
            let idx = self
                .tile_index(*tile)
                .ok_or_else(|| anyhow!("invalid tile"))?;
            if !self.tiles[idx].buildable || self.tiles[idx].block_id.is_some() {
                return Err(anyhow!("tile is not buildable or is already occupied"));
            }
        }

        let id = self.make_id(kind_name(kind));
        let mut block = make_block(id.clone(), kind, pos, dir);
        block.behavior_ref = default_behavior_for(kind).map(ToOwned::to_owned);
        if kind == BlockKind::Turret {
            block.tags.push("frontline".to_string());
        }
        self.set_tile_footprint(kind, pos, Some(id.clone()));
        self.blocks.insert(id.clone(), block);
        self.fuel_banks.insert(id.clone(), 0.0);
        self.selected_id = Some(id.clone());
        self.log(
            LogLevel::Info,
            id,
            format!("placed {kind:?} at {},{}", pos.x, pos.y),
        );
        self.recompute_networks();
        Ok(self.snapshot())
    }

    pub fn deconstruct_block(&mut self, block_id: &str) -> Result<GameSnapshot> {
        let Some(block) = self.blocks.get(block_id).cloned() else {
            return Err(anyhow!("unknown block: {block_id}"));
        };
        if block.kind == BlockKind::Core {
            return Err(anyhow!("core cannot be deconstructed"));
        }

        self.blocks.remove(block_id);
        self.set_tile_footprint(block.kind, block.pos, None);
        self.remove_block_references(block_id);
        if self.selected_id.as_deref() == Some(block_id) {
            self.selected_id = None;
        }
        self.log(
            LogLevel::Info,
            block_id.to_string(),
            format!("deconstructed {}", kind_name(block.kind)),
        );
        self.recompute_networks();
        Ok(self.snapshot())
    }

    pub fn rotate_block(&mut self, block_id: &str) -> Result<GameSnapshot> {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return Err(anyhow!("unknown block: {block_id}"));
        };
        block.dir = block.dir.rotate_cw();
        let kind = block.kind;
        let dir = block.dir;
        block.status = format!("facing {dir:?}");
        self.log(
            LogLevel::Info,
            block_id.to_string(),
            format!("rotated {} to {dir:?}", kind_name(kind)),
        );
        Ok(self.snapshot())
    }

    pub fn select_entity(&mut self, id: Option<EntityId>) -> GameSnapshot {
        self.selected_id = id;
        self.snapshot()
    }

    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            tick: self.tick,
            running: self.running,
            width: self.width,
            height: self.height,
            tiles: self.tiles.clone(),
            blocks: self.blocks.values().cloned().collect(),
            enemies: self.enemies.values().cloned().collect(),
            drones: self.drones.values().cloned().collect(),
            networks: self.networks.values().cloned().collect(),
            logs: self.logs.iter().cloned().collect(),
            selected_id: self.selected_id.clone(),
            behaviors: self
                .behaviors
                .values()
                .map(|package| self.behavior_summary_with_usage(package))
                .collect(),
            pending_jobs: self.pending_jobs.clone(),
            status: self.game_status(),
        }
    }

    fn game_status(&self) -> GameStatus {
        let core_hp = self.core_hp();
        GameStatus {
            wave: wave::current_wave(self.tick),
            next_wave_in: wave::next_wave_in(self.tick),
            core_hp,
            core_max_hp: BlockKind::Core.max_hp(),
            defeated: core_hp <= 0,
            wire_threats: self
                .enemies
                .values()
                .filter(|enemy| enemy.kind == EnemyKind::WireCutter && enemy.hp > 0)
                .count() as u32,
            damaged_wires: self
                .blocks
                .values()
                .filter(|block| {
                    block.kind == BlockKind::Wire && block.hp < BlockKind::Wire.max_hp()
                })
                .count() as u32,
            network_cpu: self.networks.values().map(|network| network.cpu_pool).sum(),
        }
    }

    fn seed_world(&mut self) {
        let core_pos = Pos { x: 30, y: 30 };
        let core_id = self.make_id("core");
        let mut core = make_block(core_id.clone(), BlockKind::Core, core_pos, Direction::East);
        core.inventory.capacity = 1000;
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

    fn tick_once(&mut self) {
        if self.core_defeated() {
            self.running = false;
            return;
        }
        self.tick += 1;
        if wave::should_spawn_wave(self.tick) {
            self.spawn_wave(wave::current_wave(self.tick));
        }

        self.recompute_networks();
        self.run_programmable_behaviors();
        self.run_block_physics();
        self.run_drones();
        self.run_enemies();
        self.cleanup_destroyed();
    }

    fn cleanup_destroyed(&mut self) {
        let dead_enemies: Vec<_> = self
            .enemies
            .values()
            .filter(|e| e.hp <= 0)
            .map(|e| e.id.clone())
            .collect();
        for id in dead_enemies {
            self.enemies.remove(&id);
            self.log(LogLevel::Info, id, "enemy destroyed".to_string());
        }

        let destroyed_blocks: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.hp <= 0 && b.kind != BlockKind::Core)
            .map(|b| (b.id.clone(), b.kind, b.pos))
            .collect();
        for (id, kind, pos) in destroyed_blocks {
            self.blocks.remove(&id);
            self.set_tile_footprint(kind, pos, None);
            self.remove_block_references(&id);
            self.log(LogLevel::Warn, id, "block destroyed".to_string());
        }
        if let Some(core_id) = self
            .blocks
            .values()
            .find(|block| block.kind == BlockKind::Core && block.hp <= 0)
            .map(|block| block.id.clone())
        {
            let should_log = self
                .blocks
                .get(&core_id)
                .map(|block| block.status != "core breached")
                .unwrap_or(false);
            if let Some(core) = self.blocks.get_mut(&core_id) {
                core.hp = 0;
                core.status = "core breached".to_string();
            }
            self.running = false;
            if should_log {
                self.log(
                    LogLevel::Error,
                    core_id,
                    "core destroyed; simulation halted".to_string(),
                );
            }
        }
        self.recompute_networks();
    }

    fn core_hp(&self) -> i32 {
        self.blocks
            .values()
            .find(|block| block.kind == BlockKind::Core)
            .map(|block| block.hp.max(0))
            .unwrap_or(0)
    }

    fn core_defeated(&self) -> bool {
        self.core_hp() <= 0
    }

    fn make_id(&mut self, prefix: &str) -> String {
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
