use anyhow::{anyhow, Result};
use std::collections::BTreeSet;
use xac_core::{BehaviorKind, Direction, ItemKind};

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

    for (index, raw_line) in source.lines().enumerate() {
        let line_no = index + 1;
        let line = strip_script_comment(raw_line).trim().to_ascii_lowercase();
        if line.is_empty() {
            continue;
        }
        statements.push(parse_script_statement(kind, line_no, &line, &mut imports)?);
    }

    let mut out = vec!["(module".to_string()];
    for import in imports {
        out.push(import.wat().to_string());
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
    DrillOutputBlocked,
    DrillMine,
    RouterPushAny,
    RouterPushDir,
    RouterPushItemDir,
    RouterOutputAvailable,
    RouterOutputItemAvailable,
    AssemblerSetRecipe,
    AssemblerCanProduce,
    AssemblerInputCount,
    AssemblerOutputCount,
    AssemblerProduce,
    TurretAmmoCount,
    TurretAttackNearest,
    TurretAttackBest,
    DronePortDispatch,
    DronePortStockCount,
    DronePortChargeDockedDrones,
    DronePortCreateDeliveryJob,
    DronePortDispatchIdleDrones,
    DroneBatteryPercent,
    DroneLogicFuelRemaining,
    DroneHasJob,
    DroneHasPendingJob,
    DroneReturnToPort,
    DroneClaimDeliveryJob,
    DroneDeliver,
    DroneIdle,
    CommonFuelRemaining,
    NetStoreGetI32,
    NetStoreSetI32,
}

impl HostImport {
    fn wat(self) -> &'static str {
        match self {
            HostImport::DrillOutputBlocked => {
                r#"  (import "xac:drill" "output_blocked" (func $output_blocked (result i32)))"#
            }
            HostImport::DrillMine => r#"  (import "xac:drill" "mine" (func $mine (result i32)))"#,
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
            HostImport::DronePortStockCount => {
                r#"  (import "xac:drone_port" "stock_count" (func $stock_count (param i32) (result i32)))"#
            }
            HostImport::DronePortChargeDockedDrones => {
                r#"  (import "xac:drone_port" "charge_docked_drones" (func $charge_docked_drones (result i32)))"#
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
            HostImport::DroneIdle => r#"  (import "xac:drone" "idle" (func $idle (result i32)))"#,
            HostImport::CommonFuelRemaining => {
                r#"  (import "xac:common" "fuel_remaining" (func $fuel_remaining (result i64)))"#
            }
            HostImport::NetStoreGetI32 => {
                r#"  (import "xac:net" "store_get_i32" (func $net_get_i32 (param i32) (result i32)))"#
            }
            HostImport::NetStoreSetI32 => {
                r#"  (import "xac:net" "store_set_i32" (func $net_set_i32 (param i32 i32) (result i32)))"#
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Condition {
    OutputBlocked,
    OutputAvailable(Direction),
    OutputItemAvailable {
        item: ItemKind,
        dir: Direction,
    },
    CanProduce,
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
    StockCountGt {
        item: ItemKind,
        value: i32,
    },
    BatteryPercentLt {
        value: i32,
    },
    LogicFuelLt {
        value: u64,
    },
    HasJob,
    HasPendingJob,
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
    Idle,
    NetSet {
        key: i32,
        value: i32,
    },
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
) -> Result<ScriptStatement> {
    let tokens: Vec<_> = line.split_whitespace().collect();
    if tokens.first() == Some(&"if") {
        let (condition, action_tokens) = parse_condition(line_no, &tokens)?;
        add_condition_import(kind, line_no, &condition, imports)?;
        let action = parse_script_action(kind, line_no, action_tokens, imports)?;
        Ok(ScriptStatement::If { condition, action })
    } else {
        Ok(ScriptStatement::Action(parse_script_action(
            kind, line_no, &tokens, imports,
        )?))
    }
}

fn parse_condition<'a>(line_no: usize, tokens: &'a [&str]) -> Result<(Condition, &'a [&'a str])> {
    match tokens {
        ["if", "output_blocked", rest @ ..] => Ok((Condition::OutputBlocked, rest)),
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
        ["if", "stock_count", item, ">", value, rest @ ..] => Ok((
            Condition::StockCountGt {
                item: parse_item_or_err(line_no, item)?,
                value: parse_i32(line_no, "stock count threshold", value)?,
            },
            rest,
        )),
        ["if", "battery_percent", "<", value, rest @ ..] => Ok((
            Condition::BatteryPercentLt {
                value: parse_i32(line_no, "battery percent threshold", value)?,
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
) -> Result<ScriptAction> {
    match tokens {
        ["return"] | ["stop"] => Ok(ScriptAction::Return),
        ["noop"] => Ok(ScriptAction::Noop),
        ["mine"] => {
            ensure_kind(kind, BehaviorKind::Drill, line_no, "mine")?;
            imports.insert(HostImport::DrillMine);
            Ok(ScriptAction::Mine)
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
        Condition::StockCountGt { .. } => {
            ensure_kind(kind, BehaviorKind::DronePort, line_no, "stock_count")?;
            imports.insert(HostImport::DronePortStockCount);
        }
        Condition::BatteryPercentLt { .. } => {
            ensure_kind(kind, BehaviorKind::CarrierDrone, line_no, "battery_percent")?;
            imports.insert(HostImport::DroneBatteryPercent);
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
        Condition::StockCountGt { item, value } => {
            format!(
                "(i32.gt_s (call $stock_count (i32.const {})) (i32.const {value}))",
                item_code(&item)
            )
        }
        Condition::BatteryPercentLt { value } => {
            format!("(i32.lt_s (call $battery_percent) (i32.const {value}))")
        }
        Condition::LogicFuelLt { value } => {
            format!("(i64.lt_u (call $logic_fuel_remaining) (i64.const {value}))")
        }
        Condition::HasJob => "(call $has_job)".to_string(),
        Condition::HasPendingJob => "(call $has_pending_job)".to_string(),
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

fn render_action(action: ScriptAction, indent: &str, out: &mut Vec<String>) {
    match action {
        ScriptAction::Return => out.push(format!("{indent}(return)")),
        ScriptAction::Noop => {}
        ScriptAction::Mine => out.push(format!("{indent}(drop (call $mine))")),
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
        ScriptAction::Idle => out.push(format!("{indent}(drop (call $idle))")),
        ScriptAction::NetSet { key, value } => out.push(format!(
            "{indent}(drop (call $net_set_i32 (i32.const {key}) (i32.const {value})))"
        )),
    }
}

fn direction_code(dir: Direction) -> i32 {
    match dir {
        Direction::North => 0,
        Direction::East => 1,
        Direction::South => 2,
        Direction::West => 3,
    }
}

fn recipe_code(recipe: &ItemKind) -> i32 {
    match recipe {
        ItemKind::Plate => 0,
        ItemKind::Ammo => 1,
        _ => 0,
    }
}

fn item_code(item: &ItemKind) -> i32 {
    match item {
        ItemKind::Ore => 0,
        ItemKind::Plate => 1,
        ItemKind::Ammo => 2,
        ItemKind::CpuPart => 3,
        ItemKind::DronePart => 4,
    }
}
