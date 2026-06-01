use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use xac_core::{Direction, EnemyKind, ItemKind, Pos};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BehaviorIntent {
    Noop,
    Drill {
        commands: Vec<DrillCommand>,
    },
    Router {
        item: Option<ItemKind>,
        preferred: Vec<Direction>,
    },
    Assembler {
        commands: Vec<AssemblerCommand>,
    },
    Turret {
        priority: Vec<TargetRule>,
    },
    TurretScanIndex {
        index: u32,
    },
    DronePort {
        commands: Vec<DronePortCommand>,
    },
    CarrierDrone {
        command: DroneCommand,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DrillCommand {
    Mine,
    Output { item: ItemKind },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AssemblerCommand {
    SetRecipe { recipe: ItemKind },
    Produce { recipe: ItemKind },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DronePortCommand {
    AutoDispatch,
    ChargeDockedDrones,
    CreateDeliveryJob {
        item: ItemKind,
        amount: u32,
        dropoff_tag: String,
    },
    DispatchIdleDrones,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DroneCommand {
    ReturnToPort,
    ClaimDeliveryJob,
    Deliver,
    MoveTo { pos: Pos },
    Load { item: ItemKind, amount: u32 },
    Unload { item: ItemKind, amount: u32 },
    Idle,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TargetRule {
    Kind(EnemyKind),
    Nearest,
    LowestHp,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorEval {
    pub intent: BehaviorIntent,
    pub net_ops: Vec<NetStoreOp>,
    pub logs: Vec<BehaviorLog>,
    pub fuel_spent: u64,
    pub fuel_remaining: u64,
    pub over_budget: bool,
    pub wasm_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BehaviorLog {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetStoreWrite {
    pub key: i32,
    pub value: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetStoreDelete {
    pub key: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NetStoreOp {
    Set(NetStoreWrite),
    Delete(NetStoreDelete),
}

#[derive(Clone, Debug, Default)]
pub struct BehaviorHostInput {
    pub output_blocked: bool,
    pub drill_ore_kind: Option<ItemKind>,
    pub can_produce: bool,
    pub assembler_can_produce: [bool; 2],
    pub assembler_current_recipe: Option<ItemKind>,
    pub assembler_input_counts: BTreeMap<ItemKind, i32>,
    pub assembler_output_counts: BTreeMap<ItemKind, i32>,
    pub ammo_count: i32,
    pub turret_visible_enemy_count: i32,
    pub turret_visible_enemy_kinds: Vec<EnemyKind>,
    pub turret_visible_enemy_hp: Vec<i32>,
    pub turret_visible_enemy_distance: Vec<f32>,
    pub router_output_available: [bool; 4],
    pub router_item_output_available: BTreeMap<ItemKind, [bool; 4]>,
    pub network_stock_counts: BTreeMap<ItemKind, i32>,
    pub network_stock_capacity: BTreeMap<ItemKind, i32>,
    pub network_stock_space: BTreeMap<ItemKind, i32>,
    pub drone_port_stock_counts: BTreeMap<ItemKind, i32>,
    pub drone_port_docked_drone_count: i32,
    pub drone_port_pending_job_count: i32,
    pub drone_battery_percent: i32,
    pub drone_logic_fuel: u64,
    pub drone_has_job: bool,
    pub drone_has_pending_job: bool,
    pub drone_cargo_counts: BTreeMap<ItemKind, i32>,
    pub net_i32: BTreeMap<i32, i32>,
    pub net_writable: bool,
}
