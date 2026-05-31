use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use xac_core::{
    BehaviorId, BehaviorSource, BehaviorSummary, Block, BlockKind, BuildResult, DeliveryJob,
    Direction, Drone, Enemy, EnemyKind, EntityId, GameSnapshot, GameStatus, ItemKind, LogEntry,
    LogLevel, Network, Pos, TerrainKind, Tile,
};
use xac_wasm::{
    BehaviorHostInput, BehaviorIntent, BehaviorRuntime, CompiledBehavior, NetStoreWrite,
};

mod behavior;
mod block_defs;
mod combat;
mod drone;
mod geometry;
mod network;

use behavior::{builtin_behaviors, BehaviorPackage};
use block_defs::{
    build_tiles, can_accept_item, cpu_scaled_threshold, default_behavior_for, kind_name, make_block,
};
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
            behaviors: builtin_behaviors(),
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
        sim.recompute_networks();
        Ok(sim)
    }

    pub fn set_running(&mut self, running: bool) -> GameSnapshot {
        self.running = running;
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

    pub fn select_entity(&mut self, id: Option<EntityId>) -> GameSnapshot {
        self.selected_id = id;
        self.snapshot()
    }

    pub fn open_behavior(&self, id: &str) -> Result<BehaviorSource> {
        let package = self
            .behaviors
            .get(id)
            .ok_or_else(|| anyhow!("unknown behavior: {id}"))?;
        Ok(BehaviorSource {
            summary: self.behavior_summary_with_usage(package),
            source: package.source.clone(),
        })
    }

    pub fn edit_builtin_copy(&mut self, block_id: &str) -> Result<BehaviorSource> {
        let behavior_id = self
            .blocks
            .get(block_id)
            .and_then(|b| b.behavior_ref.clone())
            .ok_or_else(|| anyhow!("selected block has no behavior"))?;
        let original = self
            .behaviors
            .get(&behavior_id)
            .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?
            .clone();

        if !original.summary.builtin {
            return self.open_behavior(&behavior_id);
        }

        let new_id = self.make_id("behavior");
        let display_name = format!("{} Copy", original.summary.display_name);
        let source_path = self
            .config_root
            .join("projects/default_project/blocks")
            .join(&new_id)
            .join("src/behavior.xac")
            .to_string_lossy()
            .to_string();
        let summary = BehaviorSummary {
            id: new_id.clone(),
            display_name,
            base_kind: original.summary.base_kind,
            world: original.summary.world,
            builtin: false,
            used_by: 0,
            source_path,
            build_status: "copied".to_string(),
        };
        self.behaviors.insert(
            new_id.clone(),
            BehaviorPackage {
                summary,
                source: original.source,
                wasm_hash: original.wasm_hash,
            },
        );
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.behavior_ref = Some(new_id.clone());
        }
        self.log(
            LogLevel::Info,
            block_id.to_string(),
            format!("created editable copy {new_id}"),
        );
        self.open_behavior(&new_id)
    }

    pub fn fork_behavior(&mut self, block_id: &str) -> Result<BehaviorSource> {
        let behavior_id = self
            .blocks
            .get(block_id)
            .and_then(|b| b.behavior_ref.clone())
            .ok_or_else(|| anyhow!("selected block has no behavior"))?;
        let original = self
            .behaviors
            .get(&behavior_id)
            .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?
            .clone();
        let new_id = self.make_id("behavior");
        let source_path = self
            .config_root
            .join("projects/default_project/blocks")
            .join(&new_id)
            .join("src/behavior.xac")
            .to_string_lossy()
            .to_string();
        let summary = BehaviorSummary {
            id: new_id.clone(),
            display_name: format!("{} Fork", original.summary.display_name),
            base_kind: original.summary.base_kind,
            world: original.summary.world,
            builtin: false,
            used_by: 0,
            source_path,
            build_status: "forked".to_string(),
        };
        self.behaviors.insert(
            new_id.clone(),
            BehaviorPackage {
                summary,
                source: original.source,
                wasm_hash: original.wasm_hash,
            },
        );
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.behavior_ref = Some(new_id.clone());
        }
        self.log(
            LogLevel::Info,
            block_id.to_string(),
            format!("forked behavior into {new_id}"),
        );
        self.open_behavior(&new_id)
    }

    pub fn save_behavior(&mut self, behavior_id: &str, source: String) -> Result<BehaviorSource> {
        let package = self
            .behaviors
            .get_mut(behavior_id)
            .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
        if package.summary.builtin {
            return Err(anyhow!(
                "builtin presets are read-only; create a copy first"
            ));
        }
        package.source = source;
        package.wasm_hash = None;
        package.summary.build_status = "saved".to_string();
        self.compiled_behaviors.remove(behavior_id);
        self.log(
            LogLevel::Info,
            behavior_id.to_string(),
            "source saved".to_string(),
        );
        self.open_behavior(behavior_id)
    }

    pub fn build_behavior(&mut self, behavior_id: &str) -> Result<BuildResult> {
        let (kind, source) = {
            let package = self
                .behaviors
                .get(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            (package.summary.base_kind, package.source.clone())
        };
        match self.runtime.compile_wat(kind, &source) {
            Ok(compiled) => {
                let wasm_hash = Some(compiled.wasm_hash().to_string());
                self.compiled_behaviors
                    .insert(behavior_id.to_string(), compiled);
                if let Some(package) = self.behaviors.get_mut(behavior_id) {
                    package.wasm_hash = wasm_hash.clone();
                    package.summary.build_status = "built".to_string();
                }
                self.log(
                    LogLevel::Info,
                    behavior_id.to_string(),
                    "build ok; WAT compiled to wasm".to_string(),
                );
                Ok(BuildResult {
                    behavior_id: behavior_id.to_string(),
                    success: true,
                    message: "behavior built and hot-reloaded".to_string(),
                    wasm_hash,
                })
            }
            Err(error) => {
                if let Some(package) = self.behaviors.get_mut(behavior_id) {
                    package.summary.build_status = "build failed".to_string();
                }
                self.log(LogLevel::Error, behavior_id.to_string(), error.to_string());
                Ok(BuildResult {
                    behavior_id: behavior_id.to_string(),
                    success: false,
                    message: error.to_string(),
                    wasm_hash: None,
                })
            }
        }
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
        let wave_phase = (self.tick % 80) as u32;
        let next_wave_in = if wave_phase < 20 {
            20 - wave_phase
        } else {
            100 - wave_phase
        };
        GameStatus {
            wave: (self.tick / 80) as u32 + 1,
            next_wave_in,
            wire_threats: self
                .enemies
                .values()
                .filter(|enemy| enemy.kind == EnemyKind::WireCutter && enemy.hp > 0)
                .count() as u32,
            damaged_wires: self
                .blocks
                .values()
                .filter(|block| block.kind == BlockKind::Wire && block.hp < 15)
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
        self.tick += 1;
        if self.tick % 80 == 20 {
            self.spawn_wave_enemy();
        }
        if self.tick.is_multiple_of(300) {
            self.spawn_enemy(EnemyKind::WireCutter);
        }

        self.recompute_networks();
        self.run_programmable_behaviors();
        self.run_block_physics();
        self.run_drones();
        self.run_enemies();
        self.cleanup_destroyed();
    }

    fn run_programmable_behaviors(&mut self) {
        let ids: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.kind.is_programmable() && b.active)
            .map(|b| b.id.clone())
            .collect();

        for id in ids {
            let (kind, behavior_ref, cpu_rate) = match self.blocks.get(&id) {
                Some(block) => (
                    block.kind,
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
        BehaviorHostInput {
            output_blocked: self.output_blocked(block_id),
            can_produce: self.can_produce(block_id),
            ammo_count: block.inventory.count(&ItemKind::Ammo) as i32,
            net_i32: self.network_i32_values(block.network_id),
            net_writable: block
                .network_id
                .and_then(|network_id| self.networks.get(&network_id))
                .map(|network| !network.read_only_cache)
                .unwrap_or(false),
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
        let max_bank = cpu_rate.max(1.0);
        *bank = (*bank + cpu_rate / TICKS_PER_SECOND as f32).min(max_bank);
        bank.floor() as u64
    }

    fn spend_behavior_fuel(&mut self, block_id: &str, fuel_spent: u64) {
        if let Some(bank) = self.fuel_banks.get_mut(block_id) {
            *bank = (*bank - fuel_spent as f32).max(0.0);
        }
    }

    fn compiled_behavior(
        &mut self,
        behavior_id: &str,
        expected_kind: BlockKind,
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
            BehaviorIntent::Router { preferred } => {
                let dirs = if preferred.is_empty() {
                    Direction::all().to_vec()
                } else {
                    preferred
                };
                for dir in dirs {
                    if self.transfer_from(block_id, dir, 1) {
                        break;
                    }
                }
            }
            BehaviorIntent::Assembler { prefer_ammo } => {
                if let Some(block) = self.blocks.get_mut(block_id) {
                    block.status = if prefer_ammo {
                        "recipe: ammo priority".to_string()
                    } else {
                        "recipe: plate priority".to_string()
                    };
                }
                self.run_assembler(block_id);
            }
            BehaviorIntent::Turret { priority } => self.run_turret_once(block_id, &priority),
            BehaviorIntent::DronePort => self.ensure_drone_and_job(block_id),
            BehaviorIntent::CarrierDrone => {}
        }
    }

    fn run_block_physics(&mut self) {
        let ids: Vec<_> = self.blocks.keys().cloned().collect();

        for id in ids {
            let Some(kind) = self.blocks.get(&id).map(|b| b.kind) else {
                continue;
            };
            match kind {
                BlockKind::Drill | BlockKind::Conveyor | BlockKind::Assembler => {
                    let dir = self.blocks.get(&id).map(|b| b.dir).unwrap_or_default();
                    self.transfer_from(&id, dir, 1);
                }
                BlockKind::Router => {}
                _ => {}
            }
        }
    }

    fn run_drill(&mut self, block_id: &str) {
        let mine_ready = self
            .blocks
            .get(block_id)
            .map(|b| {
                self.tile_at(b.pos)
                    .is_some_and(|t| t.terrain == TerrainKind::OrePatch)
            })
            .unwrap_or(false);
        if !mine_ready {
            return;
        }
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.progress += 1;
            let threshold = cpu_scaled_threshold(block.effective_cpu_rate, 30);
            if block.progress >= threshold && block.inventory.has_space(1) {
                block.progress = 0;
                block.inventory.add(ItemKind::Ore, 1);
                block.status = "mined ore".to_string();
            }
        }
    }

    fn run_assembler(&mut self, block_id: &str) {
        let prefer_ammo = self
            .blocks
            .get(block_id)
            .map(|b| b.status.contains("ammo"))
            .unwrap_or(true);
        let Some(block) = self.blocks.get_mut(block_id) else {
            return;
        };
        block.progress += 1;
        let threshold = cpu_scaled_threshold(block.effective_cpu_rate, 40);
        if block.progress < threshold {
            return;
        }
        block.progress = 0;
        if prefer_ammo
            && block.inventory.count(&ItemKind::Plate) >= 1
            && block.inventory.has_space(2)
        {
            block.inventory.remove(&ItemKind::Plate, 1);
            block.inventory.add(ItemKind::Ammo, 2);
            block.status = "produced ammo".to_string();
        } else if block.inventory.count(&ItemKind::Ore) >= 2 && block.inventory.has_space(1) {
            block.inventory.remove(&ItemKind::Ore, 2);
            block.inventory.add(ItemKind::Plate, 1);
            block.status = "produced plate".to_string();
        }
    }

    fn output_blocked(&self, block_id: &str) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
            return true;
        };
        if !block.inventory.has_space(1) {
            return true;
        }
        let Some(dst_id) = self.block_id_at(block.pos.step(block.dir)) else {
            return false;
        };
        !self
            .blocks
            .get(&dst_id)
            .map(|dst| can_accept_item(dst.kind, &ItemKind::Ore) && dst.inventory.has_space(1))
            .unwrap_or(false)
    }

    fn can_produce(&self, block_id: &str) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
            return false;
        };
        if block.kind != BlockKind::Assembler {
            return false;
        }
        let can_make_ammo =
            block.inventory.count(&ItemKind::Plate) >= 1 && block.inventory.has_space(2);
        let can_make_plate =
            block.inventory.count(&ItemKind::Ore) >= 2 && block.inventory.has_space(1);
        can_make_ammo || can_make_plate
    }

    fn transfer_from(&mut self, block_id: &str, dir: Direction, amount: u32) -> bool {
        let (kind, src_pos) = match self.blocks.get(block_id) {
            Some(block) => (block.kind, block.pos),
            None => return false,
        };
        let dst_pos = src_pos.step(dir);
        let Some(dst_id) = self.block_id_at(dst_pos) else {
            return false;
        };
        let Some((item_kind, available)) = self.transferable_item(block_id, &dst_id, amount) else {
            return false;
        };
        let moved = {
            let Some(src) = self.blocks.get_mut(block_id) else {
                return false;
            };
            src.inventory.remove(&item_kind, amount.min(available))
        };
        if moved == 0 {
            return false;
        }
        if let Some(dst) = self.blocks.get_mut(&dst_id) {
            dst.inventory.add(item_kind.clone(), moved);
            dst.status = format!("received {}", item_kind.as_str());
        }
        if let Some(src) = self.blocks.get_mut(block_id) {
            src.status = match kind {
                BlockKind::Router => format!("routed {}", item_kind.as_str()),
                _ => format!("sent {}", item_kind.as_str()),
            };
        }
        true
    }

    fn transferable_item(
        &self,
        src_id: &str,
        dst_id: &str,
        amount: u32,
    ) -> Option<(ItemKind, u32)> {
        let src = self.blocks.get(src_id)?;
        let dst = self.blocks.get(dst_id)?;
        if !dst.inventory.has_space(amount) {
            return None;
        }
        src.inventory
            .items
            .iter()
            .find(|(item_kind, available)| **available > 0 && can_accept_item(dst.kind, item_kind))
            .map(|(kind, amount)| (kind.clone(), *amount))
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
            self.fuel_banks.remove(&id);
            self.set_tile_footprint(kind, pos, None);
            self.log(LogLevel::Warn, id, "block destroyed".to_string());
        }
        self.recompute_networks();
    }

    fn behavior_summary_with_usage(&self, package: &BehaviorPackage) -> BehaviorSummary {
        let mut summary = package.summary.clone();
        summary.used_by = self
            .blocks
            .values()
            .filter(|b| b.behavior_ref.as_ref() == Some(&summary.id))
            .count() as u32;
        summary
    }

    fn make_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}_{}", self.next_id);
        self.next_id += 1;
        id
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
mod tests {
    use super::*;
    use xac_core::{DroneState, WorldPos};

    #[test]
    fn placing_wire_and_cpu_node_forms_network() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Wire, Pos { x: 34, y: 32 }, Direction::East)
            .unwrap();
        sim.place_block(BlockKind::CpuNode, Pos { x: 35, y: 32 }, Direction::East)
            .unwrap();
        let snapshot = sim.snapshot();
        assert!(snapshot.networks.iter().any(|n| n.cpu_pool >= 200.0));
    }

    #[test]
    fn core_occupies_four_by_four_tiles() {
        let sim = Simulation::new("/tmp/xac-test").unwrap();
        let core = sim
            .blocks
            .values()
            .find(|block| block.kind == BlockKind::Core)
            .unwrap();
        assert_eq!(core.pos, Pos { x: 30, y: 30 });
        assert_eq!(sim.block_id_at(Pos { x: 30, y: 30 }), Some(core.id.clone()));
        assert_eq!(sim.block_id_at(Pos { x: 33, y: 33 }), Some(core.id.clone()));
        assert_eq!(sim.block_id_at(Pos { x: 34, y: 33 }), None);
    }

    #[test]
    fn builtin_copy_is_editable_and_reassigned() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
            .unwrap();
        let block_id = sim.selected_id.clone().unwrap();
        let source = sim.edit_builtin_copy(&block_id).unwrap();
        assert!(!source.summary.builtin);
        assert_eq!(source.summary.used_by, 1);
    }

    #[test]
    fn minimum_devices_place_and_drill_mines_ore_with_builtin_loop_source() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();

        for x in 20..=30 {
            sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
                .unwrap();
        }
        for x in 21..30 {
            sim.place_block(BlockKind::Conveyor, Pos { x, y: 30 }, Direction::East)
                .unwrap();
        }
        sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
            .unwrap();

        let drill_id = sim.selected_id.clone().unwrap();
        let drill = sim.blocks.get(&drill_id).unwrap();
        assert_eq!(drill.kind, BlockKind::Drill);
        assert_eq!(drill.behavior_ref.as_deref(), Some("builtin.drill.basic"));

        let source = sim.open_behavior("builtin.drill.basic").unwrap();
        assert!(source.source.contains("if output_blocked return"));
        assert!(source.source.contains("mine"));

        let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
        let starting_core_ore = sim.blocks[&core_id].inventory.count(&ItemKind::Ore);
        sim.step_ticks(500);
        let delivered = sim.blocks[&core_id].inventory.count(&ItemKind::Ore);
        assert!(
            delivered > starting_core_ore,
            "drill ore should ride conveyors into the 4x4 core"
        );
    }

    #[test]
    fn behavior_build_compiles_wat_and_save_invalidates_cache() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
            .unwrap();
        let block_id = sim.selected_id.clone().unwrap();
        let source = sim.edit_builtin_copy(&block_id).unwrap();
        let behavior_id = source.summary.id;

        sim.save_behavior(&behavior_id, xac_wasm::wat_const_action(30))
            .unwrap();
        let result = sim.build_behavior(&behavior_id).unwrap();
        assert!(result.success);
        assert!(result.wasm_hash.is_some());
        assert!(sim.compiled_behaviors.contains_key(&behavior_id));

        sim.save_behavior(&behavior_id, "(module".to_string())
            .unwrap();
        assert!(!sim.compiled_behaviors.contains_key(&behavior_id));
        assert!(sim.behaviors[&behavior_id].wasm_hash.is_none());

        let result = sim.build_behavior(&behavior_id).unwrap();
        assert!(!result.success);
        assert!(result.message.contains("parse behavior source as WAT"));
    }

    #[test]
    fn xac_script_writes_network_store_and_recompute_preserves_it() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
            .unwrap();
        let router_id = sim.selected_id.clone().unwrap();
        let source = sim.edit_builtin_copy(&router_id).unwrap();
        sim.save_behavior(&source.summary.id, "net_set 7 42".to_string())
            .unwrap();
        sim.fuel_banks.insert(router_id.clone(), 100.0);

        sim.step_ticks(1);

        let network_id = sim.blocks[&router_id].network_id.unwrap();
        assert_eq!(
            sim.networks[&network_id].store.get("7"),
            Some(&serde_json::Value::from(42))
        );

        sim.recompute_networks();

        let network_id = sim.blocks[&router_id].network_id.unwrap();
        assert_eq!(
            sim.networks[&network_id].store.get("7"),
            Some(&serde_json::Value::from(42))
        );
    }

    #[test]
    fn scripted_mining_factory_feeds_turret_and_defends_core() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();

        for x in 20..=30 {
            sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
                .unwrap();
        }
        sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
            .unwrap();
        sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
            .unwrap();
        let drill_id = sim.selected_id.clone().unwrap();
        assign_script(&mut sim, &drill_id, "if output_blocked return\nmine");

        sim.place_block(BlockKind::Conveyor, Pos { x: 21, y: 30 }, Direction::East)
            .unwrap();
        sim.place_block(BlockKind::Router, Pos { x: 22, y: 30 }, Direction::East)
            .unwrap();
        let router_id = sim.selected_id.clone().unwrap();
        assign_script(&mut sim, &router_id, "push east");

        sim.place_block(BlockKind::Assembler, Pos { x: 23, y: 30 }, Direction::East)
            .unwrap();
        let assembler_id = sim.selected_id.clone().unwrap();
        assign_script(
            &mut sim,
            &assembler_id,
            "set_recipe ammo\nif can_produce produce",
        );

        sim.place_block(BlockKind::Turret, Pos { x: 24, y: 30 }, Direction::East)
            .unwrap();
        let turret_id = sim.selected_id.clone().unwrap();
        assign_script(&mut sim, &turret_id, "if ammo_count > 0 attack_nearest");

        sim.step_ticks(800);

        assert!(
            sim.blocks[&turret_id].inventory.count(&ItemKind::Ammo) > 0,
            "ore should be mined, routed, assembled into ammo, and delivered into the turret"
        );

        let enemy_id = sim.make_id("enemy");
        sim.enemies.insert(
            enemy_id.clone(),
            combat::enemy_at(
                enemy_id.clone(),
                EnemyKind::Grunt,
                WorldPos { x: 25.5, y: 30.5 },
            ),
        );

        sim.step_ticks(80);

        assert!(
            !sim.enemies.contains_key(&enemy_id),
            "scripted turret should consume factory-made ammo and destroy the nearby enemy"
        );
    }

    #[test]
    fn cpu_node_increases_wasm_driven_drill_throughput() {
        fn setup(with_cpu_node: bool) -> (Simulation, EntityId) {
            let mut sim = Simulation::new("/tmp/xac-test").unwrap();
            if with_cpu_node {
                for x in 20..=30 {
                    sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
                        .unwrap();
                }
                sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
                    .unwrap();
            }
            sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
                .unwrap();
            let drill_id = sim.selected_id.clone().unwrap();
            (sim, drill_id)
        }

        let (mut slow_sim, slow_drill_id) = setup(false);
        let (mut fast_sim, fast_drill_id) = setup(true);

        slow_sim.step_ticks(260);
        fast_sim.step_ticks(260);

        let slow_rate = slow_sim.blocks[&slow_drill_id].effective_cpu_rate;
        let fast_rate = fast_sim.blocks[&fast_drill_id].effective_cpu_rate;
        let slow_ore = slow_sim.blocks[&slow_drill_id]
            .inventory
            .count(&ItemKind::Ore);
        let fast_ore = fast_sim.blocks[&fast_drill_id]
            .inventory
            .count(&ItemKind::Ore);

        assert!(fast_rate > slow_rate);
        assert!(
            fast_ore > slow_ore,
            "cpu node should increase WAT-driven drill throughput: slow={slow_ore}, fast={fast_ore}"
        );
    }

    #[test]
    fn assembler_builtin_calls_host_api_and_produces_ammo() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Assembler, Pos { x: 34, y: 32 }, Direction::East)
            .unwrap();
        let assembler_id = sim.selected_id.clone().unwrap();
        sim.blocks
            .get_mut(&assembler_id)
            .unwrap()
            .inventory
            .add(ItemKind::Plate, 1);

        sim.step_ticks(40);

        let assembler = &sim.blocks[&assembler_id];
        assert!(
            assembler.inventory.count(&ItemKind::Ammo) > 0,
            "assembler builtin should call set_recipe/can_produce/produce through Wasm host imports"
        );
    }

    #[test]
    fn turret_builtin_calls_host_api_and_attacks_enemy() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
            .unwrap();
        let turret_id = sim.selected_id.clone().unwrap();
        sim.blocks
            .get_mut(&turret_id)
            .unwrap()
            .inventory
            .add(ItemKind::Ammo, 3);
        let enemy_id = sim.make_id("enemy");
        sim.enemies.insert(
            enemy_id.clone(),
            Enemy {
                id: enemy_id.clone(),
                kind: EnemyKind::Grunt,
                pos: WorldPos { x: 35.5, y: 32.5 },
                hp: 30,
                max_hp: 30,
                speed_ticks: 8,
                move_cooldown: 0,
                move_speed: 0.07,
                target_id: None,
            },
        );

        sim.step_ticks(40);

        let enemy_hp = sim
            .enemies
            .get(&enemy_id)
            .map(|enemy| enemy.hp)
            .unwrap_or(0);
        assert!(
            enemy_hp < 30,
            "turret builtin should call attack_nearest through Wasm host imports"
        );
    }

    #[test]
    fn drone_port_builtin_delivers_core_ammo_to_turret_and_returns_home() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
        let starting_core_ammo = sim.blocks[&core_id].inventory.count(&ItemKind::Ammo);

        sim.place_block(BlockKind::DronePort, Pos { x: 34, y: 30 }, Direction::East)
            .unwrap();
        let port_id = sim.selected_id.clone().unwrap();
        sim.place_block(BlockKind::Turret, Pos { x: 42, y: 30 }, Direction::East)
            .unwrap();
        let turret_id = sim.selected_id.clone().unwrap();

        sim.step_ticks(360);

        assert_eq!(
            sim.drones.len(),
            1,
            "one drone_port should maintain one carrier drone instead of spawning every time it leaves"
        );
        let drone = sim.drones.values().next().unwrap();
        assert_eq!(drone.home_port, port_id);
        assert_eq!(drone.state, DroneState::Docked);
        assert!(drone.job.is_none());
        assert!(
            sim.pending_jobs.is_empty(),
            "delivery job should be consumed after the turret receives ammo"
        );
        assert!(
            sim.blocks[&turret_id].inventory.count(&ItemKind::Ammo) >= 10,
            "carrier drone should deliver core ammo to the turret"
        );
        assert!(
            sim.blocks[&core_id].inventory.count(&ItemKind::Ammo) < starting_core_ammo,
            "delivery should remove ammo from core storage"
        );
    }

    #[test]
    fn wire_cutter_breaks_wire_and_splits_cpu_network() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        for x in 20..=30 {
            sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
                .unwrap();
        }
        sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
            .unwrap();
        sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
            .unwrap();
        let drill_id = sim.selected_id.clone().unwrap();
        let wire_id = sim.block_id_at(Pos { x: 20, y: 29 }).unwrap();

        sim.step_ticks(1);
        let connected_rate = sim.blocks[&drill_id].effective_cpu_rate;
        assert!(connected_rate > 100.0);

        let enemy_id = sim.make_id("enemy");
        sim.enemies.insert(
            enemy_id.clone(),
            combat::enemy_at(
                enemy_id,
                EnemyKind::WireCutter,
                WorldPos { x: 20.5, y: 29.5 },
            ),
        );
        let status = sim.snapshot().status;
        assert_eq!(status.wire_threats, 1);
        assert_eq!(status.network_cpu, 200.0);

        sim.step_ticks(4);

        assert!(
            !sim.blocks.contains_key(&wire_id),
            "wire cutter should destroy the targeted wire"
        );
        let disconnected_rate = sim.blocks[&drill_id].effective_cpu_rate;
        assert!(
            disconnected_rate < connected_rate,
            "destroying a wire should lower drill CPU by splitting the network: before={connected_rate}, after={disconnected_rate}"
        );
        assert_eq!(
            sim.blocks[&drill_id].network_id, None,
            "drill should fall back to local CPU when wire is cut"
        );
    }

    fn assign_script(sim: &mut Simulation, block_id: &str, source: &str) {
        let behavior = sim.edit_builtin_copy(block_id).unwrap();
        let result = sim
            .build_behavior(&behavior.summary.id)
            .expect("copied builtin should build");
        assert!(result.success);
        sim.save_behavior(&behavior.summary.id, source.to_string())
            .unwrap();
        let result = sim
            .build_behavior(&behavior.summary.id)
            .expect("custom XaC script should build");
        assert!(result.success);
    }
}
