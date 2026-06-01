#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum HostImport {
    CommonLog,
    DrillOutputBlocked,
    DrillMine,
    DrillOutput,
    DrillOreKind,
    RouterPushAny,
    RouterPushDir,
    RouterPushItemDir,
    RouterOutputAvailable,
    RouterOutputItemAvailable,
    AssemblerSetRecipe,
    AssemblerCurrentRecipe,
    AssemblerCanProduce,
    AssemblerInputCount,
    AssemblerOutputCount,
    AssemblerProduce,
    TurretScanEnemies,
    TurretEnemyKind,
    TurretEnemyHp,
    TurretEnemyDistance,
    TurretCanAttack,
    TurretAttack,
    TurretAmmoCount,
    TurretAttackNearest,
    TurretAttackBest,
    DronePortDispatch,
    DronePortChargeDockedDrones,
    DronePortDockedDroneCount,
    DronePortPendingJobCount,
    DronePortCreateDeliveryJob,
    DronePortDispatchIdleDrones,
    DroneBatteryPercent,
    DroneBatteryRatio,
    DroneLogicFuelRemaining,
    DroneHasJob,
    DroneHasPendingJob,
    DroneReturnToPort,
    DroneClaimDeliveryJob,
    DroneDeliver,
    DroneMoveTo,
    DroneLoad,
    DroneUnload,
    DroneCargoCount,
    DroneIdle,
    CommonFuelRemaining,
    CommonInventoryCount,
    CommonInventoryFree,
    CommonStockCount,
    CommonStockCapacity,
    CommonHasSpace,
    NetStoreGetI32,
    NetStoreSetI32,
    NetStoreDeleteI32,
}

impl HostImport {
    pub(crate) fn wat(self) -> &'static str {
        match self {
            HostImport::CommonLog => {
                r#"  (import "xac:common" "log" (func $log (param i32 i32) (result i32)))"#
            }
            HostImport::DrillOutputBlocked => {
                r#"  (import "xac:drill" "output_blocked" (func $output_blocked (result i32)))"#
            }
            HostImport::DrillMine => r#"  (import "xac:drill" "mine" (func $mine (result i32)))"#,
            HostImport::DrillOutput => {
                r#"  (import "xac:drill" "output" (func $output (param i32) (result i32)))"#
            }
            HostImport::DrillOreKind => {
                r#"  (import "xac:drill" "ore_kind" (func $ore_kind (result i32)))"#
            }
            HostImport::RouterPushAny => {
                r#"  (import "xac:router" "push_any" (func $push_any (result i32)))"#
            }
            HostImport::RouterPushDir => {
                r#"  (import "xac:router" "push_dir" (func $push_dir (param i32) (result i32)))"#
            }
            HostImport::RouterPushItemDir => {
                r#"  (import "xac:router" "push_item_dir" (func $push_item_dir (param i32 i32) (result i32)))"#
            }
            HostImport::RouterOutputAvailable => {
                r#"  (import "xac:router" "output_available" (func $output_available (param i32) (result i32)))"#
            }
            HostImport::RouterOutputItemAvailable => {
                r#"  (import "xac:router" "output_item_available" (func $output_item_available (param i32 i32) (result i32)))"#
            }
            HostImport::AssemblerSetRecipe => {
                r#"  (import "xac:assembler" "set_recipe" (func $set_recipe (param i32) (result i32)))"#
            }
            HostImport::AssemblerCurrentRecipe => {
                r#"  (import "xac:assembler" "current_recipe" (func $current_recipe (result i32)))"#
            }
            HostImport::AssemblerCanProduce => {
                r#"  (import "xac:assembler" "can_produce" (func $can_produce (result i32)))"#
            }
            HostImport::AssemblerInputCount => {
                r#"  (import "xac:assembler" "input_count" (func $input_count (param i32) (result i32)))"#
            }
            HostImport::AssemblerOutputCount => {
                r#"  (import "xac:assembler" "output_count" (func $output_count (param i32) (result i32)))"#
            }
            HostImport::AssemblerProduce => {
                r#"  (import "xac:assembler" "produce" (func $produce (result i32)))"#
            }
            HostImport::TurretScanEnemies => {
                r#"  (import "xac:turret" "scan_enemies" (func $scan_enemies (result i32)))"#
            }
            HostImport::TurretEnemyKind => {
                r#"  (import "xac:turret" "enemy_kind" (func $enemy_kind (param i32) (result i32)))"#
            }
            HostImport::TurretEnemyHp => {
                r#"  (import "xac:turret" "enemy_hp" (func $enemy_hp (param i32) (result i32)))"#
            }
            HostImport::TurretEnemyDistance => {
                r#"  (import "xac:turret" "enemy_distance" (func $enemy_distance (param i32) (result f32)))"#
            }
            HostImport::TurretCanAttack => {
                r#"  (import "xac:turret" "can_attack" (func $can_attack (param i32) (result i32)))"#
            }
            HostImport::TurretAttack => {
                r#"  (import "xac:turret" "attack" (func $attack (param i32) (result i32)))"#
            }
            HostImport::TurretAmmoCount => {
                r#"  (import "xac:turret" "ammo_count" (func $ammo_count (result i32)))"#
            }
            HostImport::TurretAttackNearest => {
                r#"  (import "xac:turret" "attack_nearest" (func $attack_nearest (result i32)))"#
            }
            HostImport::TurretAttackBest => {
                r#"  (import "xac:turret" "attack_best" (func $attack_best (param i32) (result i32)))"#
            }
            HostImport::DronePortDispatch => {
                r#"  (import "xac:drone_port" "dispatch" (func $dispatch (result i32)))"#
            }
            HostImport::DronePortChargeDockedDrones => {
                r#"  (import "xac:drone_port" "charge_docked_drones" (func $charge_docked_drones (result i32)))"#
            }
            HostImport::DronePortDockedDroneCount => {
                r#"  (import "xac:drone_port" "docked_drone_count" (func $docked_drone_count (result i32)))"#
            }
            HostImport::DronePortPendingJobCount => {
                r#"  (import "xac:drone_port" "pending_job_count" (func $pending_job_count (result i32)))"#
            }
            HostImport::DronePortCreateDeliveryJob => {
                r#"  (import "xac:drone_port" "create_delivery_job" (func $create_delivery_job (param i32 i32 i32) (result i32)))"#
            }
            HostImport::DronePortDispatchIdleDrones => {
                r#"  (import "xac:drone_port" "dispatch_idle_drones" (func $dispatch_idle_drones (result i32)))"#
            }
            HostImport::DroneBatteryPercent => {
                r#"  (import "xac:drone" "battery_percent" (func $battery_percent (result i32)))"#
            }
            HostImport::DroneBatteryRatio => {
                r#"  (import "xac:drone" "battery_ratio" (func $battery_ratio (result f32)))"#
            }
            HostImport::DroneLogicFuelRemaining => {
                r#"  (import "xac:drone" "logic_fuel_remaining" (func $logic_fuel_remaining (result i64)))"#
            }
            HostImport::DroneHasJob => {
                r#"  (import "xac:drone" "has_job" (func $has_job (result i32)))"#
            }
            HostImport::DroneHasPendingJob => {
                r#"  (import "xac:drone" "has_pending_job" (func $has_pending_job (result i32)))"#
            }
            HostImport::DroneReturnToPort => {
                r#"  (import "xac:drone" "return_to_port" (func $return_to_port (result i32)))"#
            }
            HostImport::DroneClaimDeliveryJob => {
                r#"  (import "xac:drone" "claim_delivery_job" (func $claim_delivery_job (result i32)))"#
            }
            HostImport::DroneDeliver => {
                r#"  (import "xac:drone" "deliver" (func $deliver (result i32)))"#
            }
            HostImport::DroneMoveTo => {
                r#"  (import "xac:drone" "move_to" (func $move_to (param i32 i32) (result i32)))"#
            }
            HostImport::DroneLoad => {
                r#"  (import "xac:drone" "load" (func $load (param i32 i32) (result i32)))"#
            }
            HostImport::DroneUnload => {
                r#"  (import "xac:drone" "unload" (func $unload (param i32 i32) (result i32)))"#
            }
            HostImport::DroneCargoCount => {
                r#"  (import "xac:drone" "cargo_count" (func $cargo_count (param i32) (result i32)))"#
            }
            HostImport::DroneIdle => r#"  (import "xac:drone" "idle" (func $idle (result i32)))"#,
            HostImport::CommonFuelRemaining => {
                r#"  (import "xac:common" "fuel_remaining" (func $fuel_remaining (result i64)))"#
            }
            HostImport::CommonInventoryCount => {
                r#"  (import "xac:common" "inventory_count" (func $inventory_count (param i32) (result i32)))"#
            }
            HostImport::CommonInventoryFree => {
                r#"  (import "xac:common" "inventory_free" (func $inventory_free (result i32)))"#
            }
            HostImport::CommonStockCount => {
                r#"  (import "xac:common" "stock_count" (func $stock_count (param i32) (result i32)))"#
            }
            HostImport::CommonStockCapacity => {
                r#"  (import "xac:common" "stock_capacity" (func $stock_capacity (param i32) (result i32)))"#
            }
            HostImport::CommonHasSpace => {
                r#"  (import "xac:common" "has_space" (func $has_space (param i32 i32) (result i32)))"#
            }
            HostImport::NetStoreGetI32 => {
                r#"  (import "xac:net" "store_get_i32" (func $net_get_i32 (param i32) (result i32)))"#
            }
            HostImport::NetStoreSetI32 => {
                r#"  (import "xac:net" "store_set_i32" (func $net_set_i32 (param i32 i32) (result i32)))"#
            }
            HostImport::NetStoreDeleteI32 => {
                r#"  (import "xac:net" "store_delete_i32" (func $net_delete_i32 (param i32) (result i32)))"#
            }
        }
    }
}
