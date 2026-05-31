use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type EntityId = String;
pub type BehaviorId = String;

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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyKind {
    Grunt,
    Runner,
    Armored,
    WireCutter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DroneState {
    Docked,
    Delivering,
    Returning,
    Offline,
}

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
    pub speed_ticks: u32,
    pub move_cooldown: u32,
    pub move_speed: f32,
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
