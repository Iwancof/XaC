use anyhow::{anyhow, Result};
use std::collections::BTreeSet;
use xac_core::{BehaviorKind, Direction, EnemyKind, ItemKind};

use super::abi_codes::{direction_code, enemy_kind_code, item_code, recipe_code};

const MAX_LOG_MESSAGE_BYTES: usize = 256;

pub(crate) const ATTACK_POLICY_NEAREST: i32 = 2;
pub(crate) const ATTACK_POLICY_LOWEST_HP: i32 = 3;
pub(crate) const ATTACK_POLICY_RUNNER: i32 = 4;
pub(crate) const ATTACK_POLICY_WIRE_CUTTER: i32 = 5;
pub(crate) const ATTACK_POLICY_ARMORED: i32 = 6;
pub(crate) const ATTACK_POLICY_GRUNT: i32 = 7;

pub(crate) fn is_wat_source(source: &str) -> bool {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with(";;"))
        .is_some_and(|line| line.starts_with("(module"))
}

pub(crate) fn compile_xac_script(kind: BehaviorKind, source: &str) -> Result<String> {
    let mut imports = BTreeSet::new();
    let mut statements = Vec::new();
    let mut log_data = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line_no = index + 1;
        let line = strip_script_comment(raw_line).trim().to_ascii_lowercase();
        if line.is_empty() {
            continue;
        }
        statements.push(parse_script_statement(
            kind,
            line_no,
            &line,
            &mut imports,
            &mut log_data,
        )?);
    }

    let mut out = vec!["(module".to_string()];
    for import in imports {
        out.push(import.wat().to_string());
    }
    if !log_data.is_empty() {
        out.push(r#"  (memory (export "memory") 1)"#.to_string());
        for entry in &log_data {
            out.push(format!(
                r#"  (data (i32.const {}) "{}")"#,
                entry.offset,
                wat_data_escape(&entry.bytes)
            ));
        }
    }
    out.push(r#"  (func (export "tick")"#.to_string());
    for statement in &statements {
        render_statement(statement, &mut out);
    }
    out.push("  )".to_string());
    out.push(")".to_string());
    Ok(out.join("\n"))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HostImport {
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
    fn wat(self) -> &'static str {
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

#[derive(Clone, Debug)]
enum Condition {
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
enum CountComparison {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

#[derive(Clone, Debug)]
enum ScriptAction {
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
struct LogData {
    offset: u32,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
enum ScriptStatement {
    Action(ScriptAction),
    If {
        condition: Condition,
        action: ScriptAction,
    },
}

fn strip_script_comment(line: &str) -> &str {
    let hash = line.find('#');
    let slash = line.find("//");
    match (hash, slash) {
        (Some(a), Some(b)) => &line[..a.min(b)],
        (Some(index), None) | (None, Some(index)) => &line[..index],
        (None, None) => line,
    }
}

fn parse_script_statement(
    kind: BehaviorKind,
    line_no: usize,
    line: &str,
    imports: &mut BTreeSet<HostImport>,
    log_data: &mut Vec<LogData>,
) -> Result<ScriptStatement> {
    let tokens: Vec<_> = line.split_whitespace().collect();
    if tokens.first() == Some(&"if") {
        let (condition, action_tokens) = parse_condition(line_no, &tokens)?;
        add_condition_import(kind, line_no, &condition, imports)?;
        let action = parse_script_action(kind, line_no, action_tokens, imports, log_data)?;
        Ok(ScriptStatement::If { condition, action })
    } else {
        Ok(ScriptStatement::Action(parse_script_action(
            kind, line_no, &tokens, imports, log_data,
        )?))
    }
}

fn parse_condition<'a>(line_no: usize, tokens: &'a [&str]) -> Result<(Condition, &'a [&'a str])> {
    match tokens {
        ["if", "output_blocked", rest @ ..] => Ok((Condition::OutputBlocked, rest)),
        ["if", "ore_kind", "==", item, rest @ ..] => Ok((
            Condition::OreKindEq {
                item: parse_item_or_err(line_no, item)?,
            },
            rest,
        )),
        ["if", "output_available", item, dir, rest @ ..] if parse_item(item).is_some() => Ok((
            Condition::OutputItemAvailable {
                item: parse_item(item).expect("guarded by parse_item"),
                dir: parse_direction(line_no, dir)?,
            },
            rest,
        )),
        ["if", "output_available", dir, rest @ ..] => Ok((
            Condition::OutputAvailable(parse_direction(line_no, dir)?),
            rest,
        )),
        ["if", "can_produce", rest @ ..] => Ok((Condition::CanProduce, rest)),
        ["if", "current_recipe", "==", recipe, rest @ ..] => Ok((
            Condition::CurrentRecipeEq {
                recipe: parse_recipe(line_no, recipe)?,
            },
            rest,
        )),
        ["if", "input_count", item, comparison, value, rest @ ..] => Ok((
            Condition::AssemblerInputCount {
                item: parse_item_or_err(line_no, item)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "input count threshold", value)?,
            },
            rest,
        )),
        ["if", "output_count", item, comparison, value, rest @ ..] => Ok((
            Condition::AssemblerOutputCount {
                item: parse_item_or_err(line_no, item)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "output count threshold", value)?,
            },
            rest,
        )),
        ["if", "ammo_count", ">", "0", rest @ ..] => Ok((Condition::AmmoGtZero, rest)),
        ["if", "scan_enemies", comparison, value, rest @ ..] => Ok((
            Condition::ScanEnemies {
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "scan enemy count threshold", value)?,
            },
            rest,
        )),
        ["if", "enemy_kind", index, "==", kind, rest @ ..] => Ok((
            Condition::EnemyKindEq {
                index: parse_i32(line_no, "scan enemy index", index)?,
                kind: parse_enemy_kind(line_no, kind)?,
            },
            rest,
        )),
        ["if", "enemy_hp", index, comparison, value, rest @ ..] => Ok((
            Condition::EnemyHp {
                index: parse_i32(line_no, "scan enemy index", index)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "enemy hp threshold", value)?,
            },
            rest,
        )),
        ["if", "enemy_distance", index, comparison, value, rest @ ..] => Ok((
            Condition::EnemyDistance {
                index: parse_i32(line_no, "scan enemy index", index)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_f32(line_no, "enemy distance threshold", value)?,
            },
            rest,
        )),
        ["if", "can_attack", index, rest @ ..] => Ok((
            Condition::CanAttack {
                index: parse_i32(line_no, "scan enemy index", index)?,
            },
            rest,
        )),
        ["if", "inventory_count", item, comparison, value, rest @ ..] => Ok((
            Condition::InventoryCount {
                item: parse_item_or_err(line_no, item)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "inventory count threshold", value)?,
            },
            rest,
        )),
        ["if", "inventory_free", comparison, value, rest @ ..] => Ok((
            Condition::InventoryFree {
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "inventory free threshold", value)?,
            },
            rest,
        )),
        ["if", "stock_count", item, comparison, value, rest @ ..] => Ok((
            Condition::StockCount {
                item: parse_item_or_err(line_no, item)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "stock count threshold", value)?,
            },
            rest,
        )),
        ["if", "stock_capacity", item, comparison, value, rest @ ..] => Ok((
            Condition::StockCapacity {
                item: parse_item_or_err(line_no, item)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "stock capacity threshold", value)?,
            },
            rest,
        )),
        ["if", "has_space", item, amount, rest @ ..] => Ok((
            Condition::HasSpace {
                item: parse_item_or_err(line_no, item)?,
                amount: parse_i32(line_no, "space amount", amount)?,
            },
            rest,
        )),
        ["if", "docked_drone_count", comparison, value, rest @ ..] => Ok((
            Condition::DronePortDockedDroneCount {
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "docked drone count threshold", value)?,
            },
            rest,
        )),
        ["if", "pending_job_count", comparison, value, rest @ ..] => Ok((
            Condition::DronePortPendingJobCount {
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "pending job count threshold", value)?,
            },
            rest,
        )),
        ["if", "battery_percent", "<", value, rest @ ..] => Ok((
            Condition::BatteryPercentLt {
                value: parse_i32(line_no, "battery percent threshold", value)?,
            },
            rest,
        )),
        ["if", "battery_ratio", "<", value, rest @ ..] => Ok((
            Condition::BatteryRatioLt {
                value: parse_f32(line_no, "battery ratio threshold", value)?,
            },
            rest,
        )),
        ["if", "logic_fuel_remaining", "<", value, rest @ ..] => Ok((
            Condition::LogicFuelLt {
                value: parse_u64(line_no, "logic fuel threshold", value)?,
            },
            rest,
        )),
        ["if", "has_job", rest @ ..] => Ok((Condition::HasJob, rest)),
        ["if", "has_pending_job", rest @ ..] => Ok((Condition::HasPendingJob, rest)),
        ["if", "cargo_count", item, comparison, value, rest @ ..] => Ok((
            Condition::CargoCount {
                item: parse_item_or_err(line_no, item)?,
                comparison: parse_count_comparison(line_no, comparison)?,
                value: parse_i32(line_no, "cargo count threshold", value)?,
            },
            rest,
        )),
        ["if", "fuel_remaining", ">", value, rest @ ..] => Ok((
            Condition::FuelGt {
                value: parse_u64(line_no, "fuel remaining threshold", value)?,
            },
            rest,
        )),
        ["if", "net", key, ">", value, rest @ ..] => Ok((
            Condition::NetGt {
                key: parse_i32(line_no, "network key", key)?,
                value: parse_i32(line_no, "network value", value)?,
            },
            rest,
        )),
        ["if", "net", key, "==", value, rest @ ..] => Ok((
            Condition::NetEq {
                key: parse_i32(line_no, "network key", key)?,
                value: parse_i32(line_no, "network value", value)?,
            },
            rest,
        )),
        _ => Err(anyhow!("line {line_no}: unknown condition")),
    }
}

fn parse_script_action(
    kind: BehaviorKind,
    line_no: usize,
    tokens: &[&str],
    imports: &mut BTreeSet<HostImport>,
    log_data: &mut Vec<LogData>,
) -> Result<ScriptAction> {
    match tokens {
        ["return"] | ["stop"] => Ok(ScriptAction::Return),
        ["noop"] => Ok(ScriptAction::Noop),
        ["log", message @ ..] if !message.is_empty() => {
            let message = message.join(" ");
            let (offset, len) = register_log_message(line_no, &message, log_data)?;
            imports.insert(HostImport::CommonLog);
            Ok(ScriptAction::Log { offset, len })
        }
        ["mine"] => {
            ensure_kind(kind, BehaviorKind::Drill, line_no, "mine")?;
            imports.insert(HostImport::DrillMine);
            Ok(ScriptAction::Mine)
        }
        ["output", item] => {
            ensure_kind(kind, BehaviorKind::Drill, line_no, "output")?;
            imports.insert(HostImport::DrillOutput);
            Ok(ScriptAction::Output {
                item: parse_item_or_err(line_no, item)?,
            })
        }
        ["push_any"] | ["push", "any"] => {
            ensure_kind(kind, BehaviorKind::Router, line_no, "push")?;
            imports.insert(HostImport::RouterPushAny);
            Ok(ScriptAction::PushAny)
        }
        ["push", dir] => {
            ensure_kind(kind, BehaviorKind::Router, line_no, "push")?;
            let dir = parse_direction(line_no, dir)?;
            imports.insert(HostImport::RouterPushDir);
            Ok(ScriptAction::PushDir(dir))
        }
        ["push", item, dir] if parse_item(item).is_some() => {
            ensure_kind(kind, BehaviorKind::Router, line_no, "push")?;
            let item = parse_item(item).expect("guarded by parse_item");
            let dir = parse_direction(line_no, dir)?;
            imports.insert(HostImport::RouterPushItemDir);
            Ok(ScriptAction::PushItemDir { item, dir })
        }
        ["set_recipe", recipe] => {
            ensure_kind(kind, BehaviorKind::Assembler, line_no, "set_recipe")?;
            imports.insert(HostImport::AssemblerSetRecipe);
            Ok(ScriptAction::SetRecipe {
                recipe: parse_recipe(line_no, recipe)?,
            })
        }
        ["produce"] => {
            ensure_kind(kind, BehaviorKind::Assembler, line_no, "produce")?;
            imports.insert(HostImport::AssemblerProduce);
            Ok(ScriptAction::Produce)
        }
        ["attack_nearest"] => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "attack_nearest")?;
            imports.insert(HostImport::TurretAttackNearest);
            Ok(ScriptAction::AttackNearest)
        }
        ["attack", index] => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "attack")?;
            imports.insert(HostImport::TurretAttack);
            Ok(ScriptAction::Attack {
                index: parse_i32(line_no, "scan enemy index", index)?,
            })
        }
        ["attack_best", policies @ ..] if !policies.is_empty() => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "attack_best")?;
            imports.insert(HostImport::TurretAttackBest);
            Ok(ScriptAction::AttackBest {
                policy: parse_attack_policy(line_no, policies)?,
            })
        }
        ["dispatch"] => {
            ensure_kind(kind, BehaviorKind::DronePort, line_no, "dispatch")?;
            imports.insert(HostImport::DronePortDispatch);
            Ok(ScriptAction::Dispatch)
        }
        ["charge_docked_drones"] => {
            ensure_kind(
                kind,
                BehaviorKind::DronePort,
                line_no,
                "charge_docked_drones",
            )?;
            imports.insert(HostImport::DronePortChargeDockedDrones);
            Ok(ScriptAction::ChargeDockedDrones)
        }
        ["create_delivery_job", item, amount, dropoff_tag] => {
            ensure_kind(
                kind,
                BehaviorKind::DronePort,
                line_no,
                "create_delivery_job",
            )?;
            imports.insert(HostImport::DronePortCreateDeliveryJob);
            Ok(ScriptAction::CreateDeliveryJob {
                item: parse_item_or_err(line_no, item)?,
                amount: parse_i32(line_no, "delivery amount", amount)?,
                dropoff_tag: parse_dropoff_tag(line_no, dropoff_tag)?,
            })
        }
        ["dispatch_idle_drones"] => {
            ensure_kind(
                kind,
                BehaviorKind::DronePort,
                line_no,
                "dispatch_idle_drones",
            )?;
            imports.insert(HostImport::DronePortDispatchIdleDrones);
            Ok(ScriptAction::DispatchIdleDrones)
        }
        ["return_to_port"] => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "return_to_port")?;
            imports.insert(HostImport::DroneReturnToPort);
            Ok(ScriptAction::ReturnToPort)
        }
        ["claim_delivery_job"] => {
            ensure_kind(
                kind,
                BehaviorKind::CarrierDrone,
                line_no,
                "claim_delivery_job",
            )?;
            imports.insert(HostImport::DroneClaimDeliveryJob);
            Ok(ScriptAction::ClaimDeliveryJob)
        }
        ["deliver"] => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "deliver")?;
            imports.insert(HostImport::DroneDeliver);
            Ok(ScriptAction::Deliver)
        }
        ["move_to", x, y] => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "move_to")?;
            imports.insert(HostImport::DroneMoveTo);
            Ok(ScriptAction::MoveTo {
                x: parse_i32(line_no, "move target x", x)?,
                y: parse_i32(line_no, "move target y", y)?,
            })
        }
        ["load", item, amount] => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "load")?;
            imports.insert(HostImport::DroneLoad);
            Ok(ScriptAction::Load {
                item: parse_item_or_err(line_no, item)?,
                amount: parse_i32(line_no, "load amount", amount)?,
            })
        }
        ["unload", item, amount] => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "unload")?;
            imports.insert(HostImport::DroneUnload);
            Ok(ScriptAction::Unload {
                item: parse_item_or_err(line_no, item)?,
                amount: parse_i32(line_no, "unload amount", amount)?,
            })
        }
        ["idle"] => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "idle")?;
            imports.insert(HostImport::DroneIdle);
            Ok(ScriptAction::Idle)
        }
        ["net_set", key, value] => {
            imports.insert(HostImport::NetStoreSetI32);
            Ok(ScriptAction::NetSet {
                key: parse_i32(line_no, "network key", key)?,
                value: parse_i32(line_no, "network value", value)?,
            })
        }
        ["net_delete" | "net_del", key] => {
            imports.insert(HostImport::NetStoreDeleteI32);
            Ok(ScriptAction::NetDelete {
                key: parse_i32(line_no, "network key", key)?,
            })
        }
        [] => Ok(ScriptAction::Noop),
        _ => Err(anyhow!("line {line_no}: unknown statement")),
    }
}

fn add_condition_import(
    kind: BehaviorKind,
    line_no: usize,
    condition: &Condition,
    imports: &mut BTreeSet<HostImport>,
) -> Result<()> {
    match condition {
        Condition::OutputBlocked => {
            ensure_kind(kind, BehaviorKind::Drill, line_no, "output_blocked")?;
            imports.insert(HostImport::DrillOutputBlocked);
        }
        Condition::OreKindEq { .. } => {
            ensure_kind(kind, BehaviorKind::Drill, line_no, "ore_kind")?;
            imports.insert(HostImport::DrillOreKind);
        }
        Condition::OutputAvailable(_) => {
            ensure_kind(kind, BehaviorKind::Router, line_no, "output_available")?;
            imports.insert(HostImport::RouterOutputAvailable);
        }
        Condition::OutputItemAvailable { .. } => {
            ensure_kind(kind, BehaviorKind::Router, line_no, "output_available")?;
            imports.insert(HostImport::RouterOutputItemAvailable);
        }
        Condition::CanProduce => {
            ensure_kind(kind, BehaviorKind::Assembler, line_no, "can_produce")?;
            imports.insert(HostImport::AssemblerCanProduce);
        }
        Condition::CurrentRecipeEq { .. } => {
            ensure_kind(kind, BehaviorKind::Assembler, line_no, "current_recipe")?;
            imports.insert(HostImport::AssemblerCurrentRecipe);
        }
        Condition::AssemblerInputCount { .. } => {
            ensure_kind(kind, BehaviorKind::Assembler, line_no, "input_count")?;
            imports.insert(HostImport::AssemblerInputCount);
        }
        Condition::AssemblerOutputCount { .. } => {
            ensure_kind(kind, BehaviorKind::Assembler, line_no, "output_count")?;
            imports.insert(HostImport::AssemblerOutputCount);
        }
        Condition::AmmoGtZero => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "ammo_count")?;
            imports.insert(HostImport::TurretAmmoCount);
        }
        Condition::ScanEnemies { .. } => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "scan_enemies")?;
            imports.insert(HostImport::TurretScanEnemies);
        }
        Condition::EnemyKindEq { .. } => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "enemy_kind")?;
            imports.insert(HostImport::TurretEnemyKind);
        }
        Condition::EnemyHp { .. } => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "enemy_hp")?;
            imports.insert(HostImport::TurretEnemyHp);
        }
        Condition::EnemyDistance { .. } => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "enemy_distance")?;
            imports.insert(HostImport::TurretEnemyDistance);
        }
        Condition::CanAttack { .. } => {
            ensure_kind(kind, BehaviorKind::Turret, line_no, "can_attack")?;
            imports.insert(HostImport::TurretCanAttack);
        }
        Condition::InventoryCount { .. } => {
            imports.insert(HostImport::CommonInventoryCount);
        }
        Condition::InventoryFree { .. } => {
            imports.insert(HostImport::CommonInventoryFree);
        }
        Condition::StockCount { .. } => {
            imports.insert(HostImport::CommonStockCount);
        }
        Condition::StockCapacity { .. } => {
            imports.insert(HostImport::CommonStockCapacity);
        }
        Condition::HasSpace { .. } => {
            imports.insert(HostImport::CommonHasSpace);
        }
        Condition::DronePortDockedDroneCount { .. } => {
            ensure_kind(kind, BehaviorKind::DronePort, line_no, "docked_drone_count")?;
            imports.insert(HostImport::DronePortDockedDroneCount);
        }
        Condition::DronePortPendingJobCount { .. } => {
            ensure_kind(kind, BehaviorKind::DronePort, line_no, "pending_job_count")?;
            imports.insert(HostImport::DronePortPendingJobCount);
        }
        Condition::BatteryPercentLt { .. } => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "battery_percent")?;
            imports.insert(HostImport::DroneBatteryPercent);
        }
        Condition::BatteryRatioLt { .. } => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "battery_ratio")?;
            imports.insert(HostImport::DroneBatteryRatio);
        }
        Condition::LogicFuelLt { .. } => {
            ensure_kind(
                kind,
                BehaviorKind::CarrierDrone,
                line_no,
                "logic_fuel_remaining",
            )?;
            imports.insert(HostImport::DroneLogicFuelRemaining);
        }
        Condition::HasJob => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "has_job")?;
            imports.insert(HostImport::DroneHasJob);
        }
        Condition::HasPendingJob => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "has_pending_job")?;
            imports.insert(HostImport::DroneHasPendingJob);
        }
        Condition::CargoCount { .. } => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "cargo_count")?;
            imports.insert(HostImport::DroneCargoCount);
        }
        Condition::FuelGt { .. } => {
            imports.insert(HostImport::CommonFuelRemaining);
        }
        Condition::NetGt { .. } | Condition::NetEq { .. } => {
            imports.insert(HostImport::NetStoreGetI32);
        }
    }
    Ok(())
}

fn ensure_kind(
    kind: BehaviorKind,
    expected: BehaviorKind,
    line_no: usize,
    api: &str,
) -> Result<()> {
    if kind == expected {
        Ok(())
    } else {
        Err(anyhow!(
            "line {line_no}: {api} is only available to {expected:?}, not {kind:?}"
        ))
    }
}

fn parse_direction(line_no: usize, dir: &str) -> Result<Direction> {
    match dir {
        "north" => Ok(Direction::North),
        "east" => Ok(Direction::East),
        "south" => Ok(Direction::South),
        "west" => Ok(Direction::West),
        _ => Err(anyhow!("line {line_no}: unknown direction {dir}")),
    }
}

fn parse_recipe(line_no: usize, recipe: &str) -> Result<ItemKind> {
    match recipe {
        "ammo" => Ok(ItemKind::Ammo),
        "plate" => Ok(ItemKind::Plate),
        _ => Err(anyhow!("line {line_no}: unknown recipe {recipe}")),
    }
}

fn parse_item(item: &str) -> Option<ItemKind> {
    match item {
        "ore" => Some(ItemKind::Ore),
        "plate" => Some(ItemKind::Plate),
        "ammo" => Some(ItemKind::Ammo),
        "cpu_part" | "cpu-part" => Some(ItemKind::CpuPart),
        "drone_part" | "drone-part" => Some(ItemKind::DronePart),
        _ => None,
    }
}

fn parse_item_or_err(line_no: usize, item: &str) -> Result<ItemKind> {
    parse_item(item).ok_or_else(|| anyhow!("line {line_no}: unknown item {item}"))
}

fn parse_dropoff_tag(line_no: usize, tag: &str) -> Result<i32> {
    match tag {
        "frontline" => Ok(0),
        _ => Err(anyhow!("line {line_no}: unknown dropoff tag {tag}")),
    }
}

fn parse_count_comparison(line_no: usize, comparison: &str) -> Result<CountComparison> {
    match comparison {
        "<" => Ok(CountComparison::Lt),
        "<=" => Ok(CountComparison::Le),
        "==" => Ok(CountComparison::Eq),
        ">=" => Ok(CountComparison::Ge),
        ">" => Ok(CountComparison::Gt),
        _ => Err(anyhow!(
            "line {line_no}: unknown count comparison {comparison}"
        )),
    }
}

fn parse_attack_policy(line_no: usize, policies: &[&str]) -> Result<i32> {
    let mut codes = Vec::new();
    for raw_policy in policies {
        for policy in raw_policy
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            let code = match policy {
                "nearest" => ATTACK_POLICY_NEAREST,
                "lowest_hp" | "weakest" => ATTACK_POLICY_LOWEST_HP,
                "runner" => ATTACK_POLICY_RUNNER,
                "wire_cutter" | "wire-cutter" => ATTACK_POLICY_WIRE_CUTTER,
                "armored" => ATTACK_POLICY_ARMORED,
                "grunt" => ATTACK_POLICY_GRUNT,
                _ => return Err(anyhow!("line {line_no}: unknown attack policy {policy}")),
            };
            codes.push(code);
        }
    }
    if !codes.contains(&ATTACK_POLICY_NEAREST) {
        codes.push(ATTACK_POLICY_NEAREST);
    }
    encode_attack_policy(line_no, &codes)
}

fn parse_enemy_kind(line_no: usize, kind: &str) -> Result<EnemyKind> {
    match kind {
        "grunt" => Ok(EnemyKind::Grunt),
        "runner" => Ok(EnemyKind::Runner),
        "armored" => Ok(EnemyKind::Armored),
        "wire_cutter" | "wire-cutter" => Ok(EnemyKind::WireCutter),
        _ => Err(anyhow!("line {line_no}: unknown enemy kind {kind}")),
    }
}

fn encode_attack_policy(line_no: usize, codes: &[i32]) -> Result<i32> {
    let mut encoded = 0_i32;
    for (index, code) in codes.iter().enumerate() {
        if index >= 7 {
            return Err(anyhow!("line {line_no}: attack policy is too long"));
        }
        encoded |= *code << (index * 4);
    }
    Ok(encoded)
}

fn parse_i32(line_no: usize, label: &str, value: &str) -> Result<i32> {
    value
        .parse()
        .map_err(|_| anyhow!("line {line_no}: invalid {label} {value}"))
}

fn parse_u64(line_no: usize, label: &str, value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| anyhow!("line {line_no}: invalid {label} {value}"))
}

fn parse_f32(line_no: usize, label: &str, value: &str) -> Result<f32> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| anyhow!("line {line_no}: invalid {label} {value}"))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(anyhow!("line {line_no}: invalid {label} {value}"))
    }
}

fn render_statement(statement: &ScriptStatement, out: &mut Vec<String>) {
    match statement {
        ScriptStatement::Action(action) => render_action(action.clone(), "    ", out),
        ScriptStatement::If { condition, action } => {
            out.push(format!("    (if {}", render_condition(condition.clone())));
            out.push("      (then".to_string());
            render_action(action.clone(), "        ", out);
            out.push("      ))".to_string());
        }
    }
}

fn render_condition(condition: Condition) -> String {
    match condition {
        Condition::OutputBlocked => "(call $output_blocked)".to_string(),
        Condition::OreKindEq { item } => {
            format!("(i32.eq (call $ore_kind) (i32.const {}))", item_code(&item))
        }
        Condition::OutputAvailable(dir) => {
            format!(
                "(call $output_available (i32.const {}))",
                direction_code(dir)
            )
        }
        Condition::OutputItemAvailable { item, dir } => {
            format!(
                "(call $output_item_available (i32.const {}) (i32.const {}))",
                item_code(&item),
                direction_code(dir)
            )
        }
        Condition::CanProduce => "(call $can_produce)".to_string(),
        Condition::CurrentRecipeEq { recipe } => {
            format!(
                "(i32.eq (call $current_recipe) (i32.const {}))",
                recipe_code(&recipe)
            )
        }
        Condition::AssemblerInputCount {
            item,
            comparison,
            value,
        } => render_count_condition("input_count", &item, comparison, value),
        Condition::AssemblerOutputCount {
            item,
            comparison,
            value,
        } => render_count_condition("output_count", &item, comparison, value),
        Condition::AmmoGtZero => "(i32.gt_s (call $ammo_count) (i32.const 0))".to_string(),
        Condition::ScanEnemies { comparison, value } => {
            render_scalar_count_condition("scan_enemies", comparison, value)
        }
        Condition::EnemyKindEq { index, kind } => {
            format!(
                "(i32.eq (call $enemy_kind (i32.const {index})) (i32.const {}))",
                enemy_kind_code(&kind)
            )
        }
        Condition::EnemyHp {
            index,
            comparison,
            value,
        } => render_indexed_count_condition("enemy_hp", index, comparison, value),
        Condition::EnemyDistance {
            index,
            comparison,
            value,
        } => render_indexed_f32_condition("enemy_distance", index, comparison, value),
        Condition::CanAttack { index } => {
            format!("(call $can_attack (i32.const {index}))")
        }
        Condition::InventoryCount {
            item,
            comparison,
            value,
        } => render_count_condition("inventory_count", &item, comparison, value),
        Condition::InventoryFree { comparison, value } => {
            render_scalar_count_condition("inventory_free", comparison, value)
        }
        Condition::StockCount {
            item,
            comparison,
            value,
        } => render_count_condition("stock_count", &item, comparison, value),
        Condition::StockCapacity {
            item,
            comparison,
            value,
        } => render_count_condition("stock_capacity", &item, comparison, value),
        Condition::HasSpace { item, amount } => {
            format!(
                "(call $has_space (i32.const {}) (i32.const {amount}))",
                item_code(&item)
            )
        }
        Condition::DronePortDockedDroneCount { comparison, value } => {
            render_scalar_count_condition("docked_drone_count", comparison, value)
        }
        Condition::DronePortPendingJobCount { comparison, value } => {
            render_scalar_count_condition("pending_job_count", comparison, value)
        }
        Condition::BatteryPercentLt { value } => {
            format!("(i32.lt_s (call $battery_percent) (i32.const {value}))")
        }
        Condition::BatteryRatioLt { value } => {
            format!("(f32.lt (call $battery_ratio) (f32.const {value}))")
        }
        Condition::LogicFuelLt { value } => {
            format!("(i64.lt_u (call $logic_fuel_remaining) (i64.const {value}))")
        }
        Condition::HasJob => "(call $has_job)".to_string(),
        Condition::HasPendingJob => "(call $has_pending_job)".to_string(),
        Condition::CargoCount {
            item,
            comparison,
            value,
        } => render_count_condition("cargo_count", &item, comparison, value),
        Condition::FuelGt { value } => {
            format!("(i64.gt_u (call $fuel_remaining) (i64.const {value}))")
        }
        Condition::NetGt { key, value } => {
            format!("(i32.gt_s (call $net_get_i32 (i32.const {key})) (i32.const {value}))")
        }
        Condition::NetEq { key, value } => {
            format!("(i32.eq (call $net_get_i32 (i32.const {key})) (i32.const {value}))")
        }
    }
}

fn render_scalar_count_condition(
    function_name: &str,
    comparison: CountComparison,
    value: i32,
) -> String {
    let op = match comparison {
        CountComparison::Lt => "i32.lt_s",
        CountComparison::Le => "i32.le_s",
        CountComparison::Eq => "i32.eq",
        CountComparison::Ge => "i32.ge_s",
        CountComparison::Gt => "i32.gt_s",
    };
    format!("({op} (call ${function_name}) (i32.const {value}))")
}

fn render_count_condition(
    function_name: &str,
    item: &ItemKind,
    comparison: CountComparison,
    value: i32,
) -> String {
    let op = match comparison {
        CountComparison::Lt => "i32.lt_s",
        CountComparison::Le => "i32.le_s",
        CountComparison::Eq => "i32.eq",
        CountComparison::Ge => "i32.ge_s",
        CountComparison::Gt => "i32.gt_s",
    };
    format!(
        "({op} (call ${function_name} (i32.const {})) (i32.const {value}))",
        item_code(item)
    )
}

fn render_indexed_count_condition(
    function_name: &str,
    index: i32,
    comparison: CountComparison,
    value: i32,
) -> String {
    let op = match comparison {
        CountComparison::Lt => "i32.lt_s",
        CountComparison::Le => "i32.le_s",
        CountComparison::Eq => "i32.eq",
        CountComparison::Ge => "i32.ge_s",
        CountComparison::Gt => "i32.gt_s",
    };
    format!("({op} (call ${function_name} (i32.const {index})) (i32.const {value}))")
}

fn render_indexed_f32_condition(
    function_name: &str,
    index: i32,
    comparison: CountComparison,
    value: f32,
) -> String {
    let op = match comparison {
        CountComparison::Lt => "f32.lt",
        CountComparison::Le => "f32.le",
        CountComparison::Eq => "f32.eq",
        CountComparison::Ge => "f32.ge",
        CountComparison::Gt => "f32.gt",
    };
    format!("({op} (call ${function_name} (i32.const {index})) (f32.const {value}))")
}

fn render_action(action: ScriptAction, indent: &str, out: &mut Vec<String>) {
    match action {
        ScriptAction::Return => out.push(format!("{indent}(return)")),
        ScriptAction::Noop => {}
        ScriptAction::Mine => out.push(format!("{indent}(drop (call $mine))")),
        ScriptAction::Output { item } => out.push(format!(
            "{indent}(drop (call $output (i32.const {})))",
            item_code(&item)
        )),
        ScriptAction::PushAny => out.push(format!("{indent}(drop (call $push_any))")),
        ScriptAction::PushDir(dir) => out.push(format!(
            "{indent}(drop (call $push_dir (i32.const {})))",
            direction_code(dir)
        )),
        ScriptAction::PushItemDir { item, dir } => out.push(format!(
            "{indent}(drop (call $push_item_dir (i32.const {}) (i32.const {})))",
            item_code(&item),
            direction_code(dir)
        )),
        ScriptAction::SetRecipe { recipe } => out.push(format!(
            "{indent}(drop (call $set_recipe (i32.const {})))",
            recipe_code(&recipe)
        )),
        ScriptAction::Produce => out.push(format!("{indent}(drop (call $produce))")),
        ScriptAction::AttackNearest => out.push(format!("{indent}(drop (call $attack_nearest))")),
        ScriptAction::AttackBest { policy } => out.push(format!(
            "{indent}(drop (call $attack_best (i32.const {})))",
            policy
        )),
        ScriptAction::Attack { index } => out.push(format!(
            "{indent}(drop (call $attack (i32.const {index})))"
        )),
        ScriptAction::Dispatch => out.push(format!("{indent}(drop (call $dispatch))")),
        ScriptAction::ChargeDockedDrones => {
            out.push(format!("{indent}(drop (call $charge_docked_drones))"))
        }
        ScriptAction::CreateDeliveryJob {
            item,
            amount,
            dropoff_tag,
        } => out.push(format!(
            "{indent}(drop (call $create_delivery_job (i32.const {}) (i32.const {amount}) (i32.const {dropoff_tag})))",
            item_code(&item)
        )),
        ScriptAction::DispatchIdleDrones => {
            out.push(format!("{indent}(drop (call $dispatch_idle_drones))"))
        }
        ScriptAction::ReturnToPort => out.push(format!("{indent}(drop (call $return_to_port))")),
        ScriptAction::ClaimDeliveryJob => {
            out.push(format!("{indent}(drop (call $claim_delivery_job))"))
        }
        ScriptAction::Deliver => out.push(format!("{indent}(drop (call $deliver))")),
        ScriptAction::MoveTo { x, y } => out.push(format!(
            "{indent}(drop (call $move_to (i32.const {x}) (i32.const {y})))"
        )),
        ScriptAction::Load { item, amount } => out.push(format!(
            "{indent}(drop (call $load (i32.const {}) (i32.const {amount})))",
            item_code(&item)
        )),
        ScriptAction::Unload { item, amount } => out.push(format!(
            "{indent}(drop (call $unload (i32.const {}) (i32.const {amount})))",
            item_code(&item)
        )),
        ScriptAction::Idle => out.push(format!("{indent}(drop (call $idle))")),
        ScriptAction::Log { offset, len } => out.push(format!(
            "{indent}(drop (call $log (i32.const {offset}) (i32.const {len})))"
        )),
        ScriptAction::NetSet { key, value } => out.push(format!(
            "{indent}(drop (call $net_set_i32 (i32.const {key}) (i32.const {value})))"
        )),
        ScriptAction::NetDelete { key } => {
            out.push(format!("{indent}(drop (call $net_delete_i32 (i32.const {key})))"))
        }
    }
}

fn register_log_message(
    line_no: usize,
    message: &str,
    log_data: &mut Vec<LogData>,
) -> Result<(u32, u32)> {
    let bytes = message.as_bytes();
    if bytes.len() > MAX_LOG_MESSAGE_BYTES {
        return Err(anyhow!(
            "line {line_no}: log message must be at most {MAX_LOG_MESSAGE_BYTES} bytes"
        ));
    }
    let offset = log_data
        .last()
        .map(|entry| entry.offset as usize + entry.bytes.len())
        .unwrap_or(0);
    let end = offset
        .checked_add(bytes.len())
        .ok_or_else(|| anyhow!("line {line_no}: log data offset overflow"))?;
    if end > u16::MAX as usize {
        return Err(anyhow!("line {line_no}: too much static log data"));
    }
    let offset = u32::try_from(offset).expect("guarded by static log data limit");
    let len = u32::try_from(bytes.len()).expect("log message length fits u32");
    log_data.push(LogData {
        offset,
        bytes: bytes.to_vec(),
    });
    Ok((offset, len))
}

fn wat_data_escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(*byte)),
            _ => out.push_str(&format!("\\{byte:02x}")),
        }
    }
    out
}
