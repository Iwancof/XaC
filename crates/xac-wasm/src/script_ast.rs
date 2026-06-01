use xac_core::{Direction, EnemyKind, ItemKind};

#[derive(Clone, Debug)]
pub(crate) enum Condition {
    OutputBlocked,
    OreKindEq {
        item: ItemKind,
    },
    OutputAvailable(Direction),
    OutputItemAvailable {
        item: ItemKind,
        dir: Direction,
    },
    CanProduce,
    CurrentRecipeEq {
        recipe: ItemKind,
    },
    AssemblerInputCount {
        item: ItemKind,
        comparison: CountComparison,
        value: i32,
    },
    AssemblerOutputCount {
        item: ItemKind,
        comparison: CountComparison,
        value: i32,
    },
    AmmoGtZero,
    ScanEnemies {
        comparison: CountComparison,
        value: i32,
    },
    EnemyKindEq {
        index: i32,
        kind: EnemyKind,
    },
    EnemyHp {
        index: i32,
        comparison: CountComparison,
        value: i32,
    },
    EnemyDistance {
        index: i32,
        comparison: CountComparison,
        value: f32,
    },
    CanAttack {
        index: i32,
    },
    InventoryCount {
        item: ItemKind,
        comparison: CountComparison,
        value: i32,
    },
    InventoryFree {
        comparison: CountComparison,
        value: i32,
    },
    StockCount {
        item: ItemKind,
        comparison: CountComparison,
        value: i32,
    },
    StockCapacity {
        item: ItemKind,
        comparison: CountComparison,
        value: i32,
    },
    HasSpace {
        item: ItemKind,
        amount: i32,
    },
    DronePortDockedDroneCount {
        comparison: CountComparison,
        value: i32,
    },
    DronePortPendingJobCount {
        comparison: CountComparison,
        value: i32,
    },
    BatteryPercentLt {
        value: i32,
    },
    BatteryRatioLt {
        value: f32,
    },
    LogicFuelLt {
        value: u64,
    },
    HasJob,
    HasPendingJob,
    CargoCount {
        item: ItemKind,
        comparison: CountComparison,
        value: i32,
    },
    FuelGt {
        value: u64,
    },
    NetGt {
        key: i32,
        value: i32,
    },
    NetEq {
        key: i32,
        value: i32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CountComparison {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

#[derive(Clone, Debug)]
pub(crate) enum ScriptAction {
    Return,
    Noop,
    Mine,
    Output {
        item: ItemKind,
    },
    PushAny,
    PushDir(Direction),
    PushItemDir {
        item: ItemKind,
        dir: Direction,
    },
    SetRecipe {
        recipe: ItemKind,
    },
    Produce,
    AttackNearest,
    AttackBest {
        policy: i32,
    },
    Attack {
        index: i32,
    },
    Dispatch,
    ChargeDockedDrones,
    CreateDeliveryJob {
        item: ItemKind,
        amount: i32,
        dropoff_tag: i32,
    },
    DispatchIdleDrones,
    ReturnToPort,
    ClaimDeliveryJob,
    Deliver,
    MoveTo {
        x: i32,
        y: i32,
    },
    Load {
        item: ItemKind,
        amount: i32,
    },
    Unload {
        item: ItemKind,
        amount: i32,
    },
    Idle,
    Log {
        offset: u32,
        len: u32,
    },
    NetSet {
        key: i32,
        value: i32,
    },
    NetDelete {
        key: i32,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct LogData {
    pub(crate) offset: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) enum ScriptStatement {
    Action(ScriptAction),
    If {
        condition: Condition,
        action: ScriptAction,
    },
}
