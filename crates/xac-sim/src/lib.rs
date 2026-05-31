use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use xac_core::{
    BehaviorId, BehaviorSource, BehaviorSummary, Block, BlockKind, BuildResult, DeliveryJob,
    Direction, Drone, DroneState, Enemy, EnemyKind, EntityId, GameSnapshot, Inventory, ItemKind,
    LogEntry, LogLevel, Network, Pos, TerrainKind, Tile,
};
use xac_wasm::{hash_source, BehaviorIntent, BehaviorRuntime, TargetRule};

pub const MAP_WIDTH: i32 = 64;
pub const MAP_HEIGHT: i32 = 64;
pub const TICKS_PER_SECOND: u32 = 20;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorPackage {
    pub summary: BehaviorSummary,
    pub source: String,
    pub wasm_hash: Option<String>,
}

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
        if !self.in_bounds(pos) {
            return Err(anyhow!("position is outside the map"));
        }
        let idx = self
            .tile_index(pos)
            .ok_or_else(|| anyhow!("invalid tile"))?;
        if !self.tiles[idx].buildable || self.tiles[idx].block_id.is_some() {
            return Err(anyhow!("tile is not buildable or is already occupied"));
        }

        let id = self.make_id(kind_name(kind));
        let mut block = make_block(id.clone(), kind, pos, dir);
        block.behavior_ref = default_behavior_for(kind).map(ToOwned::to_owned);
        if kind == BlockKind::Turret {
            block.tags.push("frontline".to_string());
        }
        self.tiles[idx].block_id = Some(id.clone());
        self.blocks.insert(id.clone(), block);
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
            .join("src/behavior.rs")
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
            .join("src/behavior.rs")
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
        package.summary.build_status = "saved".to_string();
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
        let fuel = 30;
        match self.runtime.evaluate(kind, &source, fuel) {
            Ok(eval) => {
                let wasm_hash = Some(eval.wasm_hash.clone());
                if let Some(package) = self.behaviors.get_mut(behavior_id) {
                    package.wasm_hash = wasm_hash.clone();
                    package.summary.build_status = "built".to_string();
                }
                self.log(
                    LogLevel::Info,
                    behavior_id.to_string(),
                    format!("build ok; fuel spent {}", eval.fuel_spent),
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
        }
    }

    fn seed_world(&mut self) {
        let core_pos = Pos { x: 32, y: 32 };
        let core_id = self.make_id("core");
        let mut core = make_block(core_id.clone(), BlockKind::Core, core_pos, Direction::East);
        core.inventory.capacity = 1000;
        core.inventory.add(ItemKind::Ore, 40);
        core.inventory.add(ItemKind::Plate, 20);
        core.inventory.add(ItemKind::Ammo, 60);
        core.status = "core online".to_string();
        self.set_tile_block(core_pos, Some(core_id.clone()));
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
        if self.tick % 300 == 0 {
            self.spawn_enemy(EnemyKind::WireCutter);
        }

        self.recompute_networks();
        self.run_programmable_behaviors();
        self.run_block_physics();
        self.run_drones();
        self.run_turrets();
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
            let source = match self.behaviors.get(&behavior_ref) {
                Some(package) => package.source.clone(),
                None => continue,
            };
            let fuel = ((cpu_rate / TICKS_PER_SECOND as f32).ceil() as u64).max(1);
            match self.runtime.evaluate(kind, &source, fuel) {
                Ok(eval) => {
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
                    self.apply_behavior_intent(&id, eval.intent);
                }
                Err(error) => {
                    if let Some(block) = self.blocks.get_mut(&id) {
                        block.status = "runtime error".to_string();
                    }
                    self.log(LogLevel::Error, id, error.to_string());
                }
            }
        }
    }

    fn apply_behavior_intent(&mut self, block_id: &str, intent: BehaviorIntent) {
        match intent {
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
            }
            BehaviorIntent::DronePort => self.ensure_drone_and_job(block_id),
            BehaviorIntent::Turret { .. }
            | BehaviorIntent::DrillDefault
            | BehaviorIntent::CarrierDrone => {}
        }
    }

    fn run_block_physics(&mut self) {
        let ids: Vec<_> = self.blocks.keys().cloned().collect();

        for id in ids.clone() {
            let Some(kind) = self.blocks.get(&id).map(|b| b.kind) else {
                continue;
            };
            match kind {
                BlockKind::Drill => self.run_drill(&id),
                BlockKind::Assembler => self.run_assembler(&id),
                _ => {}
            }
        }

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

    fn run_turrets(&mut self) {
        let turret_ids: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.kind == BlockKind::Turret && b.inventory.count(&ItemKind::Ammo) > 0)
            .map(|b| b.id.clone())
            .collect();
        for turret_id in turret_ids {
            let Some(turret) = self.blocks.get(&turret_id).cloned() else {
                continue;
            };
            let priority = turret
                .behavior_ref
                .as_ref()
                .and_then(|id| self.behaviors.get(id))
                .map(|package| infer_turret_rules(&package.source))
                .unwrap_or_else(|| vec![TargetRule::Nearest]);
            let target = self.choose_target(turret.pos, &priority);
            if let Some(enemy_id) = target {
                if let Some(enemy) = self.enemies.get_mut(&enemy_id) {
                    enemy.hp -= 12;
                }
                if let Some(block) = self.blocks.get_mut(&turret_id) {
                    block.inventory.remove(&ItemKind::Ammo, 1);
                    block.status = format!("attacking {enemy_id}");
                }
            }
        }
    }

    fn run_enemies(&mut self) {
        let core_pos = self
            .blocks
            .values()
            .find(|b| b.kind == BlockKind::Core)
            .map(|b| b.pos)
            .unwrap_or(Pos { x: 32, y: 32 });
        let enemy_ids: Vec<_> = self.enemies.keys().cloned().collect();
        let blocks_snapshot = self.blocks.clone();

        for enemy_id in enemy_ids {
            let Some(enemy) = self.enemies.get_mut(&enemy_id) else {
                continue;
            };
            if enemy.move_cooldown > 0 {
                enemy.move_cooldown -= 1;
                continue;
            }
            enemy.move_cooldown = enemy.speed_ticks;
            let target_pos = if enemy.kind == EnemyKind::WireCutter {
                nearest_block_pos(&blocks_snapshot, enemy.pos, |kind| {
                    matches!(
                        kind,
                        BlockKind::Wire | BlockKind::CpuNode | BlockKind::DronePort
                    )
                })
                .unwrap_or(core_pos)
            } else {
                core_pos
            };

            if enemy.pos.manhattan(target_pos) <= 1 {
                if let Some(block_id) = block_at_snapshot(&blocks_snapshot, target_pos) {
                    if let Some(block) = self.blocks.get_mut(&block_id) {
                        block.hp -= if enemy.kind == EnemyKind::Armored {
                            8
                        } else {
                            5
                        };
                    }
                }
            } else {
                enemy.pos = step_toward(enemy.pos, target_pos);
            }
        }
    }

    fn run_drones(&mut self) {
        let port_ids: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.kind == BlockKind::DronePort)
            .map(|b| b.id.clone())
            .collect();
        for port_id in port_ids {
            self.ensure_drone_and_job(&port_id);
        }

        let drone_ids: Vec<_> = self.drones.keys().cloned().collect();
        for drone_id in drone_ids {
            let job_needed = self
                .drones
                .get(&drone_id)
                .and_then(|d| d.job.clone())
                .is_none();
            if job_needed {
                if let Some(job) = self.pending_jobs.pop() {
                    if let Some(drone) = self.drones.get_mut(&drone_id) {
                        drone.job = Some(job);
                        drone.state = DroneState::Delivering;
                    }
                }
            }

            let Some(job) = self.drones.get(&drone_id).and_then(|d| d.job.clone()) else {
                continue;
            };
            let pickup_pos = self.blocks.get(&job.pickup).map(|b| b.pos);
            let dropoff_pos = self.blocks.get(&job.dropoff).map(|b| b.pos);
            let Some(dropoff_pos) = dropoff_pos else {
                continue;
            };

            let mut completed = false;
            if let Some(drone) = self.drones.get_mut(&drone_id) {
                drone.battery = (drone.battery - 0.1).max(0.0);
                drone.logic_fuel = drone.logic_fuel.saturating_sub(1);
                if drone.cargo.count(&job.item) == 0 {
                    if let Some(pickup_pos) = pickup_pos {
                        if drone.pos == pickup_pos {
                            let loaded = self
                                .blocks
                                .get_mut(&job.pickup)
                                .map(|b| b.inventory.remove(&job.item, job.amount))
                                .unwrap_or(0);
                            drone.cargo.add(job.item.clone(), loaded);
                        } else {
                            drone.pos = step_toward(drone.pos, pickup_pos);
                        }
                    }
                } else if drone.pos == dropoff_pos {
                    let delivered = drone.cargo.remove(&job.item, job.amount);
                    if let Some(block) = self.blocks.get_mut(&job.dropoff) {
                        block.inventory.add(job.item.clone(), delivered);
                        block.status = format!("drone delivered {}", job.item.as_str());
                    }
                    completed = true;
                } else {
                    drone.pos = step_toward(drone.pos, dropoff_pos);
                }
            }
            if completed {
                if let Some(drone) = self.drones.get_mut(&drone_id) {
                    drone.job = None;
                    drone.state = DroneState::Docked;
                }
            }
        }
    }

    fn ensure_drone_and_job(&mut self, port_id: &str) {
        let Some(port) = self.blocks.get(port_id).cloned() else {
            return;
        };
        if !self.drones.values().any(|d| d.pos == port.pos) {
            let id = self.make_id("drone");
            self.drones.insert(
                id.clone(),
                Drone {
                    id,
                    pos: port.pos,
                    battery: 100.0,
                    logic_fuel: 1000,
                    cargo: Inventory::with_capacity(20),
                    state: DroneState::Docked,
                    job: None,
                },
            );
        }
        if self.tick % 60 != 0 {
            return;
        }
        let Some(dropoff) = self
            .blocks
            .values()
            .find(|b| b.kind == BlockKind::Turret && b.inventory.count(&ItemKind::Ammo) < 10)
            .map(|b| b.id.clone())
        else {
            return;
        };
        let Some(pickup) = self
            .blocks
            .values()
            .find(|b| {
                matches!(
                    b.kind,
                    BlockKind::Storage | BlockKind::Core | BlockKind::Assembler
                ) && b.inventory.count(&ItemKind::Ammo) >= 5
            })
            .map(|b| b.id.clone())
        else {
            return;
        };
        let job_id = self.make_id("job");
        self.pending_jobs.push(DeliveryJob {
            id: job_id,
            item: ItemKind::Ammo,
            amount: 10,
            pickup,
            dropoff,
            priority: 50,
        });
    }

    fn transfer_from(&mut self, block_id: &str, dir: Direction, amount: u32) -> bool {
        let (kind, src_pos, item) = match self.blocks.get(block_id) {
            Some(block) => (block.kind, block.pos, block.inventory.first_item()),
            None => return false,
        };
        let Some((item_kind, available)) = item else {
            return false;
        };
        let dst_pos = src_pos.step(dir);
        let Some(dst_id) = self.block_id_at(dst_pos) else {
            return false;
        };
        let can_accept = self
            .blocks
            .get(&dst_id)
            .map(|dst| can_accept_item(dst.kind, &item_kind) && dst.inventory.has_space(amount))
            .unwrap_or(false);
        if !can_accept {
            return false;
        }
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

    fn choose_target(&self, origin: Pos, priority: &[TargetRule]) -> Option<EntityId> {
        let in_range: Vec<_> = self
            .enemies
            .values()
            .filter(|e| e.hp > 0 && origin.manhattan(e.pos) <= 8)
            .collect();
        if in_range.is_empty() {
            return None;
        }
        for rule in priority {
            match rule {
                TargetRule::Kind(kind) => {
                    if let Some(enemy) = in_range
                        .iter()
                        .filter(|e| e.kind == *kind)
                        .min_by_key(|e| origin.manhattan(e.pos))
                    {
                        return Some(enemy.id.clone());
                    }
                }
                TargetRule::LowestHp => {
                    if let Some(enemy) = in_range.iter().min_by_key(|e| e.hp) {
                        return Some(enemy.id.clone());
                    }
                }
                TargetRule::Nearest => {
                    if let Some(enemy) = in_range.iter().min_by_key(|e| origin.manhattan(e.pos)) {
                        return Some(enemy.id.clone());
                    }
                }
            }
        }
        None
    }

    fn recompute_networks(&mut self) {
        for block in self.blocks.values_mut() {
            block.network_id = None;
            block.effective_cpu_rate = block.kind.local_cpu_rate();
            block.active = block.kind.is_programmable();
        }
        self.networks.clear();

        let network_nodes: BTreeSet<_> = self
            .blocks
            .values()
            .filter(|b| b.kind.is_network_node())
            .map(|b| b.pos)
            .collect();
        let mut seen = BTreeSet::new();
        let mut next_network = 1_u32;

        for start in network_nodes.iter().copied() {
            if seen.contains(&start) {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            let mut component = Vec::new();
            seen.insert(start);
            while let Some(pos) = queue.pop_front() {
                component.push(pos);
                for dir in Direction::all() {
                    let next = pos.step(dir);
                    if network_nodes.contains(&next) && seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }

            let block_ids: Vec<_> = component
                .iter()
                .filter_map(|pos| self.block_id_at(*pos))
                .collect();
            let cpu_pool = block_ids
                .iter()
                .filter_map(|id| self.blocks.get(id))
                .map(|b| match b.kind {
                    BlockKind::Core => 120.0,
                    BlockKind::CpuNode => 80.0,
                    BlockKind::DronePort => 20.0,
                    _ => 0.0,
                })
                .sum::<f32>();
            let active_devices = block_ids
                .iter()
                .filter_map(|id| self.blocks.get(id))
                .filter(|b| b.kind.is_programmable())
                .count() as u32;
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
            self.networks.insert(
                next_network,
                Network {
                    id: next_network,
                    cpu_pool,
                    active_devices,
                    effective_per_device,
                    block_ids,
                    store: BTreeMap::new(),
                    read_only_cache: !component.iter().any(|pos| {
                        self.block_id_at(*pos)
                            .and_then(|id| self.blocks.get(&id).map(|b| b.kind == BlockKind::Core))
                            .unwrap_or(false)
                    }),
                },
            );
            next_network += 1;
        }
    }

    fn spawn_wave_enemy(&mut self) {
        let kind = match (self.tick / 80) % 4 {
            0 => EnemyKind::Grunt,
            1 => EnemyKind::Runner,
            2 => EnemyKind::Armored,
            _ => EnemyKind::Grunt,
        };
        self.spawn_enemy(kind);
    }

    fn spawn_enemy(&mut self, kind: EnemyKind) {
        let id = self.make_id("enemy");
        let lane = (self.tick as i32 / 40) % 20;
        let pos = Pos { x: 4 + lane, y: 4 };
        let (hp, speed_ticks) = match kind {
            EnemyKind::Grunt => (30, 8),
            EnemyKind::Runner => (20, 3),
            EnemyKind::Armored => (90, 12),
            EnemyKind::WireCutter => (38, 5),
        };
        self.enemies.insert(
            id.clone(),
            Enemy {
                id: id.clone(),
                kind,
                pos,
                hp,
                max_hp: hp,
                speed_ticks,
                move_cooldown: 0,
                target_id: None,
            },
        );
        self.log(LogLevel::Warn, id, format!("{kind:?} wave contact"));
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
            .map(|b| (b.id.clone(), b.pos))
            .collect();
        for (id, pos) in destroyed_blocks {
            self.blocks.remove(&id);
            self.set_tile_block(pos, None);
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

    fn set_tile_block(&mut self, pos: Pos, block_id: Option<EntityId>) {
        if let Some(idx) = self.tile_index(pos) {
            self.tiles[idx].block_id = block_id;
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

fn build_tiles() -> Vec<Tile> {
    let mut tiles = Vec::with_capacity((MAP_WIDTH * MAP_HEIGHT) as usize);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let ore = ((x - 20).pow(2) + (y - 30).pow(2) < 42)
                || ((x - 42).pow(2) + (y - 25).pow(2) < 30)
                || ((x - 30).pow(2) + (y - 44).pow(2) < 28);
            tiles.push(Tile {
                pos: Pos { x, y },
                terrain: if ore {
                    TerrainKind::OrePatch
                } else {
                    TerrainKind::Ground
                },
                buildable: true,
                enemy_passable: true,
                block_id: None,
            });
        }
    }
    tiles
}

fn make_block(id: EntityId, kind: BlockKind, pos: Pos, dir: Direction) -> Block {
    let capacity = match kind {
        BlockKind::Core => 1000,
        BlockKind::Storage => 300,
        BlockKind::Conveyor | BlockKind::Router => 1,
        BlockKind::Turret => 80,
        BlockKind::Assembler => 100,
        BlockKind::Drill => 10,
        BlockKind::DronePort => 120,
        _ => 0,
    };
    Block {
        id,
        kind,
        pos,
        dir,
        hp: match kind {
            BlockKind::Wire => 15,
            BlockKind::Core => 500,
            _ => 90,
        },
        inventory: Inventory::with_capacity(capacity),
        behavior_ref: None,
        tags: Vec::new(),
        active: kind.is_programmable(),
        network_id: None,
        effective_cpu_rate: kind.local_cpu_rate(),
        progress: 0,
        status: "idle".to_string(),
    }
}

fn kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Core => "core",
        BlockKind::Wire => "wire",
        BlockKind::CpuNode => "cpu_node",
        BlockKind::Drill => "drill",
        BlockKind::Conveyor => "conveyor",
        BlockKind::Router => "router",
        BlockKind::Storage => "storage",
        BlockKind::Assembler => "assembler",
        BlockKind::Turret => "turret",
        BlockKind::DronePort => "drone_port",
    }
}

fn default_behavior_for(kind: BlockKind) -> Option<&'static str> {
    match kind {
        BlockKind::Drill => Some("builtin.drill.basic"),
        BlockKind::Router => Some("builtin.router.basic"),
        BlockKind::Assembler => Some("builtin.assembler.basic"),
        BlockKind::Turret => Some("builtin.turret.basic"),
        BlockKind::DronePort => Some("builtin.drone_port.basic"),
        _ => None,
    }
}

fn builtin_behaviors() -> BTreeMap<BehaviorId, BehaviorPackage> {
    let mut packages = BTreeMap::new();
    for (id, display_name, base_kind, world, source) in [
        (
            "builtin.drill.basic",
            "Basic Drill",
            BlockKind::Drill,
            "drill-behavior",
            include_str!("../../../assets/builtin/drill/basic.rs"),
        ),
        (
            "builtin.router.basic",
            "Basic Router",
            BlockKind::Router,
            "router-behavior",
            include_str!("../../../assets/builtin/router/basic.rs"),
        ),
        (
            "builtin.router.ammo_east",
            "Ammo East Router",
            BlockKind::Router,
            "router-behavior",
            include_str!("../../../assets/builtin/router/ammo_east.rs"),
        ),
        (
            "builtin.assembler.basic",
            "Basic Assembler",
            BlockKind::Assembler,
            "assembler-behavior",
            include_str!("../../../assets/builtin/assembler/basic.rs"),
        ),
        (
            "builtin.turret.basic",
            "Basic Turret",
            BlockKind::Turret,
            "turret-behavior",
            include_str!("../../../assets/builtin/turret/basic.rs"),
        ),
        (
            "builtin.turret.priority",
            "Priority Turret",
            BlockKind::Turret,
            "turret-behavior",
            include_str!("../../../assets/builtin/turret/priority.rs"),
        ),
        (
            "builtin.drone_port.basic",
            "Basic Drone Port",
            BlockKind::DronePort,
            "drone-port-behavior",
            include_str!("../../../assets/builtin/drone_port/basic.rs"),
        ),
    ] {
        let id = id.to_string();
        packages.insert(
            id.clone(),
            BehaviorPackage {
                summary: BehaviorSummary {
                    id: id.clone(),
                    display_name: display_name.to_string(),
                    base_kind,
                    world: world.to_string(),
                    builtin: true,
                    used_by: 0,
                    source_path: format!("assets/builtin/{}/behavior.rs", display_name),
                    build_status: "builtin".to_string(),
                },
                source: source.to_string(),
                wasm_hash: Some(hash_source(source)),
            },
        );
    }
    packages
}

fn can_accept_item(kind: BlockKind, item: &ItemKind) -> bool {
    match kind {
        BlockKind::Wire | BlockKind::CpuNode => false,
        BlockKind::Turret => item == &ItemKind::Ammo,
        BlockKind::Conveyor | BlockKind::Router => true,
        BlockKind::Assembler => matches!(item, ItemKind::Ore | ItemKind::Plate),
        BlockKind::Core | BlockKind::Storage | BlockKind::DronePort => true,
        BlockKind::Drill => false,
    }
}

fn cpu_scaled_threshold(effective_cpu_rate: f32, base: u32) -> u32 {
    let speedup = (effective_cpu_rate / 8.0).clamp(0.1, 10.0);
    ((base as f32 / speedup).ceil() as u32).max(3)
}

fn step_toward(from: Pos, to: Pos) -> Pos {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx.abs() > dy.abs() {
        Pos {
            x: from.x + dx.signum(),
            y: from.y,
        }
    } else if dy != 0 {
        Pos {
            x: from.x,
            y: from.y + dy.signum(),
        }
    } else {
        from
    }
}

fn nearest_block_pos(
    blocks: &BTreeMap<EntityId, Block>,
    origin: Pos,
    predicate: impl Fn(BlockKind) -> bool,
) -> Option<Pos> {
    blocks
        .values()
        .filter(|block| predicate(block.kind))
        .min_by_key(|block| origin.manhattan(block.pos))
        .map(|block| block.pos)
}

fn block_at_snapshot(blocks: &BTreeMap<EntityId, Block>, pos: Pos) -> Option<EntityId> {
    blocks
        .values()
        .find(|block| block.pos == pos)
        .map(|block| block.id.clone())
}

fn infer_turret_rules(source: &str) -> Vec<TargetRule> {
    let lower = source.to_ascii_lowercase();
    let mut rules = Vec::new();
    for (needle, kind) in [
        ("runner", EnemyKind::Runner),
        ("wire_cutter", EnemyKind::WireCutter),
        ("wire-cutter", EnemyKind::WireCutter),
        ("armored", EnemyKind::Armored),
        ("grunt", EnemyKind::Grunt),
    ] {
        if lower.contains(needle) {
            rules.push(TargetRule::Kind(kind));
        }
    }
    rules.push(TargetRule::Nearest);
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placing_wire_and_cpu_node_forms_network() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Wire, Pos { x: 33, y: 32 }, Direction::East)
            .unwrap();
        sim.place_block(BlockKind::CpuNode, Pos { x: 34, y: 32 }, Direction::East)
            .unwrap();
        let snapshot = sim.snapshot();
        assert!(snapshot.networks.iter().any(|n| n.cpu_pool >= 200.0));
    }

    #[test]
    fn builtin_copy_is_editable_and_reassigned() {
        let mut sim = Simulation::new("/tmp/xac-test").unwrap();
        sim.place_block(BlockKind::Turret, Pos { x: 33, y: 32 }, Direction::East)
            .unwrap();
        let block_id = sim.selected_id.clone().unwrap();
        let source = sim.edit_builtin_copy(&block_id).unwrap();
        assert!(!source.summary.builtin);
        assert_eq!(source.summary.used_by, 1);
    }
}
