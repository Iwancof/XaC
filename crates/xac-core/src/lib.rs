use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type EntityId = String;
pub type BehaviorId = String;

pub const CPU_SPEED_REFERENCE_RATE: f32 = 8.0;
pub const MIN_CPU_SCALED_TICKS: u32 = 3;

pub fn cpu_scaled_ticks(effective_cpu_rate: f32, base_ticks: u32) -> u32 {
    let speedup = (effective_cpu_rate / CPU_SPEED_REFERENCE_RATE).clamp(0.1, 10.0);
    ((base_ticks as f32 / speedup).ceil() as u32).max(MIN_CPU_SCALED_TICKS)
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub fn step(self, dir: Direction) -> Self {
        let (dx, dy) = dir.delta();
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub fn manhattan(self, other: Pos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorldPos {
    pub x: f32,
    pub y: f32,
}

impl WorldPos {
    pub fn from_tile_center(pos: Pos) -> Self {
        Self {
            x: pos.x as f32 + 0.5,
            y: pos.y as f32 + 0.5,
        }
    }

    pub fn tile(self) -> Pos {
        Pos {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
        }
    }

    pub fn distance(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn move_toward(self, target: Self, max_distance: f32) -> Self {
        let dx = target.x - self.x;
        let dy = target.y - self.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance <= max_distance || distance == 0.0 {
            target
        } else {
            let scale = max_distance / distance;
            Self {
                x: self.x + dx * scale,
                y: self.y + dy * scale,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    #[default]
    East,
    South,
    West,
}

impl Direction {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::East => (1, 0),
            Direction::South => (0, 1),
            Direction::West => (-1, 0),
        }
    }

    pub fn all() -> [Direction; 4] {
        [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ]
    }

    pub fn rotate_cw(self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Ore,
    Plate,
    Ammo,
    CpuPart,
    DronePart,
}

impl ItemKind {
    pub fn all() -> [Self; 5] {
        [
            ItemKind::Ore,
            ItemKind::Plate,
            ItemKind::Ammo,
            ItemKind::CpuPart,
            ItemKind::DronePart,
        ]
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "ore" => Some(ItemKind::Ore),
            "plate" => Some(ItemKind::Plate),
            "ammo" => Some(ItemKind::Ammo),
            "cpu_part" => Some(ItemKind::CpuPart),
            "drone_part" => Some(ItemKind::DronePart),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ItemKind::Ore => "ore",
            ItemKind::Plate => "plate",
            ItemKind::Ammo => "ammo",
            ItemKind::CpuPart => "cpu_part",
            ItemKind::DronePart => "drone_part",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainKind {
    Ground,
    OrePatch,
    Rock,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Core,
    Wire,
    CpuNode,
    Drill,
    Conveyor,
    Router,
    Storage,
    Assembler,
    Turret,
    DronePort,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorKind {
    Drill,
    Router,
    Assembler,
    Turret,
    DronePort,
    CarrierDrone,
}

impl BehaviorKind {
    pub fn from_block_kind(kind: BlockKind) -> Option<Self> {
        match kind {
            BlockKind::Drill => Some(BehaviorKind::Drill),
            BlockKind::Router => Some(BehaviorKind::Router),
            BlockKind::Assembler => Some(BehaviorKind::Assembler),
            BlockKind::Turret => Some(BehaviorKind::Turret),
            BlockKind::DronePort => Some(BehaviorKind::DronePort),
            _ => None,
        }
    }
}

impl BlockKind {
    pub fn as_str(self) -> &'static str {
        match self {
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

    pub fn default_behavior_id(self) -> Option<&'static str> {
        match self {
            BlockKind::Drill => Some("builtin.drill.basic"),
            BlockKind::Router => Some("builtin.router.basic"),
            BlockKind::Assembler => Some("builtin.assembler.basic"),
            BlockKind::Turret => Some("builtin.turret.basic"),
            BlockKind::DronePort => Some("builtin.drone_port.basic"),
            _ => None,
        }
    }

    pub fn is_programmable(self) -> bool {
        matches!(
            self,
            BlockKind::Drill
                | BlockKind::Router
                | BlockKind::Assembler
                | BlockKind::Turret
                | BlockKind::DronePort
        )
    }

    pub fn is_network_node(self) -> bool {
        matches!(
            self,
            BlockKind::Core
                | BlockKind::Wire
                | BlockKind::CpuNode
                | BlockKind::Drill
                | BlockKind::Router
                | BlockKind::Assembler
                | BlockKind::Turret
                | BlockKind::DronePort
                | BlockKind::Storage
        )
    }

    pub fn local_cpu_rate(self) -> f32 {
        match self {
            BlockKind::Router | BlockKind::Drill => 1.0,
            BlockKind::Assembler => 2.0,
            BlockKind::Turret | BlockKind::DronePort => 3.0,
            _ => 0.0,
        }
    }

    pub fn network_cpu_output(self) -> f32 {
        match self {
            BlockKind::Core => 120.0,
            BlockKind::CpuNode => 80.0,
            BlockKind::DronePort => 20.0,
            _ => 0.0,
        }
    }

    pub fn max_hp(self) -> i32 {
        match self {
            BlockKind::Wire => 15,
            BlockKind::Core => 500,
            _ => 90,
        }
    }

    pub fn footprint_size(self) -> (i32, i32) {
        match self {
            BlockKind::Core => (4, 4),
            _ => (1, 1),
        }
    }

    pub fn is_network_connector(self) -> bool {
        matches!(
            self,
            BlockKind::Core | BlockKind::Wire | BlockKind::CpuNode | BlockKind::DronePort
        )
    }

    pub fn inventory_capacity(self) -> u32 {
        match self {
            BlockKind::Core => 1000,
            BlockKind::Storage => 300,
            BlockKind::Conveyor | BlockKind::Router => 1,
            BlockKind::Turret => 80,
            BlockKind::Assembler => 100,
            BlockKind::Drill => 10,
            BlockKind::DronePort => 120,
            BlockKind::Wire | BlockKind::CpuNode => 0,
        }
    }

    pub fn can_accept_item(self, item: &ItemKind) -> bool {
        match self {
            BlockKind::Wire | BlockKind::CpuNode | BlockKind::Drill => false,
            BlockKind::Turret => item == &ItemKind::Ammo,
            BlockKind::Conveyor | BlockKind::Router => true,
            BlockKind::Assembler => matches!(item, ItemKind::Ore | ItemKind::Plate),
            BlockKind::Core | BlockKind::Storage | BlockKind::DronePort => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyKind {
    Grunt,
    Runner,
    Armored,
    WireCutter,
}

impl EnemyKind {
    pub fn max_hp(self) -> i32 {
        match self {
            EnemyKind::Grunt => 30,
            EnemyKind::Runner => 20,
            EnemyKind::Armored => 90,
            EnemyKind::WireCutter => 38,
        }
    }

    pub fn attack_cooldown_ticks(self) -> u32 {
        match self {
            EnemyKind::Grunt => 20,
            EnemyKind::Runner => 12,
            EnemyKind::Armored => 28,
            EnemyKind::WireCutter => 16,
        }
    }

    pub fn move_speed(self) -> f32 {
        match self {
            EnemyKind::Grunt => 0.07,
            EnemyKind::Runner => 0.14,
            EnemyKind::Armored => 0.045,
            EnemyKind::WireCutter => 0.10,
        }
    }

    pub fn attack_damage(self) -> i32 {
        match self {
            EnemyKind::Armored => 8,
            _ => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DroneState {
    Docked,
    Delivering,
    Returning,
    Offline,
}

pub const CARRIER_DRONE_LOCAL_CPU_RATE: f32 = 4.0;
pub const CARRIER_DRONE_BATTERY_CAPACITY: f32 = 100.0;
pub const CARRIER_DRONE_LOGIC_FUEL_CAPACITY: u64 = 1000;
pub const CARRIER_DRONE_CARGO_CAPACITY: u32 = 20;
pub const CARRIER_DRONE_MOVE_SPEED: f32 = 0.18;
pub const CARRIER_DRONE_MOVE_BATTERY_COST: f32 = 0.05;
pub const CARRIER_DRONE_WORK_BATTERY_COST: f32 = 0.1;
pub const CARRIER_DRONE_DOCKING_DISTANCE: f32 = 0.15;
pub const CARRIER_DRONE_BATTERY_RECHARGE_PER_TICK: f32 = 1.0;
pub const CARRIER_DRONE_LOGIC_RECHARGE_PER_TICK: u64 = 10;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Inventory {
    pub items: BTreeMap<ItemKind, u32>,
    pub capacity: u32,
}

impl Inventory {
    pub fn with_capacity(capacity: u32) -> Self {
        Self {
            capacity,
            items: BTreeMap::new(),
        }
    }

    pub fn total(&self) -> u32 {
        self.items.values().sum()
    }

    pub fn count(&self, kind: &ItemKind) -> u32 {
        *self.items.get(kind).unwrap_or(&0)
    }

    pub fn has_space(&self, amount: u32) -> bool {
        self.total().saturating_add(amount) <= self.capacity
    }

    pub fn add(&mut self, kind: ItemKind, amount: u32) -> u32 {
        let free = self.capacity.saturating_sub(self.total());
        let accepted = free.min(amount);
        if accepted > 0 {
            *self.items.entry(kind).or_insert(0) += accepted;
        }
        accepted
    }

    pub fn remove(&mut self, kind: &ItemKind, amount: u32) -> u32 {
        let current = self.count(kind);
        let removed = current.min(amount);
        if removed == 0 {
            return 0;
        }

        if let Some(entry) = self.items.get_mut(kind) {
            *entry -= removed;
            if *entry == 0 {
                self.items.remove(kind);
            }
        }
        removed
    }

    pub fn first_item(&self) -> Option<(ItemKind, u32)> {
        self.items
            .iter()
            .find(|(_, amount)| **amount > 0)
            .map(|(kind, amount)| (kind.clone(), *amount))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tile {
    pub pos: Pos,
    pub terrain: TerrainKind,
    pub buildable: bool,
    pub enemy_passable: bool,
    pub block_id: Option<EntityId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
    pub id: EntityId,
    pub kind: BlockKind,
    pub pos: Pos,
    pub dir: Direction,
    pub hp: i32,
    pub inventory: Inventory,
    pub recipe: Option<String>,
    pub behavior_ref: Option<BehaviorId>,
    pub tags: Vec<String>,
    pub active: bool,
    pub network_id: Option<u32>,
    pub effective_cpu_rate: f32,
    pub progress: u32,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Enemy {
    pub id: EntityId,
    pub kind: EnemyKind,
    pub pos: WorldPos,
    pub hp: i32,
    pub max_hp: i32,
    pub move_speed: f32,
    pub attack_cooldown: u32,
    pub target_id: Option<EntityId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeliveryJob {
    pub id: EntityId,
    pub item: ItemKind,
    pub amount: u32,
    pub pickup: EntityId,
    pub dropoff: EntityId,
    pub priority: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Drone {
    pub id: EntityId,
    pub home_port: EntityId,
    pub behavior_ref: Option<BehaviorId>,
    pub pos: WorldPos,
    pub battery: f32,
    pub logic_fuel: u64,
    pub cargo: Inventory,
    pub state: DroneState,
    pub job: Option<DeliveryJob>,
}

impl Drone {
    pub fn carrier(
        id: EntityId,
        home_port: EntityId,
        behavior_ref: Option<BehaviorId>,
        pos: WorldPos,
    ) -> Self {
        Self {
            id,
            home_port,
            behavior_ref,
            pos,
            battery: CARRIER_DRONE_BATTERY_CAPACITY,
            logic_fuel: CARRIER_DRONE_LOGIC_FUEL_CAPACITY,
            cargo: Inventory::with_capacity(CARRIER_DRONE_CARGO_CAPACITY),
            state: DroneState::Docked,
            job: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Network {
    pub id: u32,
    pub cpu_pool: f32,
    pub active_devices: u32,
    pub effective_per_device: f32,
    pub block_ids: Vec<EntityId>,
    pub store: BTreeMap<String, serde_json::Value>,
    pub read_only_cache: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorSummary {
    pub id: BehaviorId,
    pub display_name: String,
    pub base_kind: BehaviorKind,
    pub world: String,
    pub builtin: bool,
    pub used_by: u32,
    pub source_path: String,
    pub build_status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorSource {
    pub summary: BehaviorSummary,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildResult {
    pub behavior_id: BehaviorId,
    pub success: bool,
    pub message: String,
    pub wasm_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogEntry {
    pub tick: u64,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameStatus {
    pub wave: u32,
    pub next_wave_in: u32,
    pub core_hp: i32,
    pub core_max_hp: i32,
    pub defeated: bool,
    pub wire_threats: u32,
    pub damaged_wires: u32,
    pub network_cpu: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameSnapshot {
    pub tick: u64,
    pub running: bool,
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
    pub blocks: Vec<Block>,
    pub enemies: Vec<Enemy>,
    pub drones: Vec<Drone>,
    pub networks: Vec<Network>,
    pub logs: Vec<LogEntry>,
    pub selected_id: Option<EntityId>,
    pub behaviors: Vec<BehaviorSummary>,
    pub pending_jobs: Vec<DeliveryJob>,
    pub status: GameStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_scaled_ticks_uses_core_speed_contract() {
        assert_eq!(cpu_scaled_ticks(8.0, 30), 30);
        assert_eq!(cpu_scaled_ticks(16.0, 30), 15);
        assert_eq!(cpu_scaled_ticks(4.0, 30), 60);
        assert_eq!(cpu_scaled_ticks(800.0, 20), MIN_CPU_SCALED_TICKS);
        assert_eq!(cpu_scaled_ticks(0.0, 20), 200);
    }

    #[test]
    fn mvp_block_storage_contract_matches_factory_roles() {
        assert_eq!(BlockKind::Core.inventory_capacity(), 1000);
        assert_eq!(BlockKind::Storage.inventory_capacity(), 300);
        assert_eq!(BlockKind::Conveyor.inventory_capacity(), 1);
        assert_eq!(BlockKind::Router.inventory_capacity(), 1);
        assert_eq!(BlockKind::Drill.inventory_capacity(), 10);
        assert_eq!(BlockKind::Assembler.inventory_capacity(), 100);
        assert_eq!(BlockKind::Turret.inventory_capacity(), 80);
        assert_eq!(BlockKind::DronePort.inventory_capacity(), 120);
        assert_eq!(BlockKind::Wire.inventory_capacity(), 0);
        assert_eq!(BlockKind::CpuNode.inventory_capacity(), 0);

        assert!(BlockKind::Core.can_accept_item(&ItemKind::CpuPart));
        assert!(BlockKind::Storage.can_accept_item(&ItemKind::Ore));
        assert!(BlockKind::Conveyor.can_accept_item(&ItemKind::Ammo));
        assert!(BlockKind::Router.can_accept_item(&ItemKind::Plate));
        assert!(BlockKind::DronePort.can_accept_item(&ItemKind::DronePart));

        assert!(BlockKind::Assembler.can_accept_item(&ItemKind::Ore));
        assert!(BlockKind::Assembler.can_accept_item(&ItemKind::Plate));
        assert!(!BlockKind::Assembler.can_accept_item(&ItemKind::Ammo));

        assert!(BlockKind::Turret.can_accept_item(&ItemKind::Ammo));
        assert!(!BlockKind::Turret.can_accept_item(&ItemKind::Ore));

        assert!(!BlockKind::Drill.can_accept_item(&ItemKind::Ore));
        assert!(!BlockKind::Wire.can_accept_item(&ItemKind::Ore));
        assert!(!BlockKind::CpuNode.can_accept_item(&ItemKind::Ore));
    }

    #[test]
    fn block_kind_identity_and_default_behavior_contracts_are_centralized() {
        assert_eq!(BlockKind::Core.as_str(), "core");
        assert_eq!(BlockKind::Wire.as_str(), "wire");
        assert_eq!(BlockKind::CpuNode.as_str(), "cpu_node");
        assert_eq!(BlockKind::Drill.as_str(), "drill");
        assert_eq!(BlockKind::Conveyor.as_str(), "conveyor");
        assert_eq!(BlockKind::Router.as_str(), "router");
        assert_eq!(BlockKind::Storage.as_str(), "storage");
        assert_eq!(BlockKind::Assembler.as_str(), "assembler");
        assert_eq!(BlockKind::Turret.as_str(), "turret");
        assert_eq!(BlockKind::DronePort.as_str(), "drone_port");

        assert_eq!(
            BlockKind::Drill.default_behavior_id(),
            Some("builtin.drill.basic")
        );
        assert_eq!(
            BlockKind::Router.default_behavior_id(),
            Some("builtin.router.basic")
        );
        assert_eq!(
            BlockKind::Assembler.default_behavior_id(),
            Some("builtin.assembler.basic")
        );
        assert_eq!(
            BlockKind::Turret.default_behavior_id(),
            Some("builtin.turret.basic")
        );
        assert_eq!(
            BlockKind::DronePort.default_behavior_id(),
            Some("builtin.drone_port.basic")
        );
        assert_eq!(BlockKind::Core.default_behavior_id(), None);
        assert_eq!(BlockKind::Conveyor.default_behavior_id(), None);
        assert_eq!(BlockKind::Wire.default_behavior_id(), None);
    }

    #[test]
    fn carrier_drone_constructor_uses_mvp_unit_contract() {
        let drone = Drone::carrier(
            "drone_1".to_string(),
            "drone_port_1".to_string(),
            Some("builtin.carrier_drone.basic".to_string()),
            WorldPos { x: 4.5, y: 8.5 },
        );

        assert_eq!(drone.home_port, "drone_port_1");
        assert_eq!(
            drone.behavior_ref.as_deref(),
            Some("builtin.carrier_drone.basic")
        );
        assert_eq!(drone.pos, WorldPos { x: 4.5, y: 8.5 });
        assert_eq!(drone.battery, CARRIER_DRONE_BATTERY_CAPACITY);
        assert_eq!(drone.logic_fuel, CARRIER_DRONE_LOGIC_FUEL_CAPACITY);
        assert_eq!(drone.cargo.capacity, CARRIER_DRONE_CARGO_CAPACITY);
        assert_eq!(drone.state, DroneState::Docked);
        assert!(drone.job.is_none());
    }
}
