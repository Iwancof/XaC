use anyhow::{anyhow, Result};
use std::collections::BTreeSet;
use xac_core::{BehaviorKind, Direction, EnemyKind, ItemKind};

use super::script::{
    ATTACK_POLICY_ARMORED, ATTACK_POLICY_GRUNT, ATTACK_POLICY_LOWEST_HP, ATTACK_POLICY_NEAREST,
    ATTACK_POLICY_RUNNER, ATTACK_POLICY_WIRE_CUTTER,
};
use super::script_ast::{Condition, CountComparison, LogData, ScriptAction, ScriptStatement};
use super::script_imports::HostImport;

const MAX_LOG_MESSAGE_BYTES: usize = 256;

pub(crate) struct ParsedScript {
    pub(crate) imports: BTreeSet<HostImport>,
    pub(crate) statements: Vec<ScriptStatement>,
    pub(crate) log_data: Vec<LogData>,
}

pub(crate) fn parse_script_source(kind: BehaviorKind, source: &str) -> Result<ParsedScript> {
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

    Ok(ParsedScript {
        imports,
        statements,
        log_data,
    })
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
