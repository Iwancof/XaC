use anyhow::{anyhow, Result};
use std::collections::BTreeSet;
use xac_core::{BlockKind, Direction, ItemKind};

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

pub(crate) fn compile_xac_script(kind: BlockKind, source: &str) -> Result<String> {
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
    RouterOutputAvailable,
    AssemblerSetRecipe,
    AssemblerCanProduce,
    AssemblerProduce,
    TurretAmmoCount,
    TurretAttackNearest,
    TurretAttackBest,
    DronePortDispatch,
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
            HostImport::RouterOutputAvailable => {
                r#"  (import "xac:router" "output_available" (func $output_available (param i32) (result i32)))"#
            }
            HostImport::AssemblerSetRecipe => {
                r#"  (import "xac:assembler" "set_recipe" (func $set_recipe (param i32) (result i32)))"#
            }
            HostImport::AssemblerCanProduce => {
                r#"  (import "xac:assembler" "can_produce" (func $can_produce (result i32)))"#
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

#[derive(Clone, Copy, Debug)]
enum Condition {
    OutputBlocked,
    OutputAvailable(Direction),
    CanProduce,
    AmmoGtZero,
    FuelGt { value: u64 },
    NetGt { key: i32, value: i32 },
    NetEq { key: i32, value: i32 },
}

#[derive(Clone, Debug)]
enum ScriptAction {
    Return,
    Noop,
    Mine,
    PushAny,
    PushDir(Direction),
    SetRecipe { recipe: ItemKind },
    Produce,
    AttackNearest,
    AttackBest { policy: i32 },
    Dispatch,
    NetSet { key: i32, value: i32 },
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
    kind: BlockKind,
    line_no: usize,
    line: &str,
    imports: &mut BTreeSet<HostImport>,
) -> Result<ScriptStatement> {
    let tokens: Vec<_> = line.split_whitespace().collect();
    if tokens.first() == Some(&"if") {
        let (condition, action_tokens) = parse_condition(line_no, &tokens)?;
        add_condition_import(kind, line_no, condition, imports)?;
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
        ["if", "output_available", dir, rest @ ..] => Ok((
            Condition::OutputAvailable(parse_direction(line_no, dir)?),
            rest,
        )),
        ["if", "can_produce", rest @ ..] => Ok((Condition::CanProduce, rest)),
        ["if", "ammo_count", ">", "0", rest @ ..] => Ok((Condition::AmmoGtZero, rest)),
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
    kind: BlockKind,
    line_no: usize,
    tokens: &[&str],
    imports: &mut BTreeSet<HostImport>,
) -> Result<ScriptAction> {
    match tokens {
        ["return"] | ["stop"] => Ok(ScriptAction::Return),
        ["noop"] => Ok(ScriptAction::Noop),
        ["mine"] => {
            ensure_kind(kind, BlockKind::Drill, line_no, "mine")?;
            imports.insert(HostImport::DrillMine);
            Ok(ScriptAction::Mine)
        }
        ["push_any"] | ["push", "any"] => {
            ensure_kind(kind, BlockKind::Router, line_no, "push")?;
            imports.insert(HostImport::RouterPushAny);
            Ok(ScriptAction::PushAny)
        }
        ["push", dir] => {
            ensure_kind(kind, BlockKind::Router, line_no, "push")?;
            let dir = parse_direction(line_no, dir)?;
            imports.insert(HostImport::RouterPushDir);
            Ok(ScriptAction::PushDir(dir))
        }
        ["set_recipe", recipe] => {
            ensure_kind(kind, BlockKind::Assembler, line_no, "set_recipe")?;
            imports.insert(HostImport::AssemblerSetRecipe);
            Ok(ScriptAction::SetRecipe {
                recipe: parse_recipe(line_no, recipe)?,
            })
        }
        ["produce"] => {
            ensure_kind(kind, BlockKind::Assembler, line_no, "produce")?;
            imports.insert(HostImport::AssemblerProduce);
            Ok(ScriptAction::Produce)
        }
        ["attack_nearest"] => {
            ensure_kind(kind, BlockKind::Turret, line_no, "attack_nearest")?;
            imports.insert(HostImport::TurretAttackNearest);
            Ok(ScriptAction::AttackNearest)
        }
        ["attack_best", policies @ ..] if !policies.is_empty() => {
            ensure_kind(kind, BlockKind::Turret, line_no, "attack_best")?;
            imports.insert(HostImport::TurretAttackBest);
            Ok(ScriptAction::AttackBest {
                policy: parse_attack_policy(line_no, policies)?,
            })
        }
        ["dispatch"] => {
            ensure_kind(kind, BlockKind::DronePort, line_no, "dispatch")?;
            imports.insert(HostImport::DronePortDispatch);
            Ok(ScriptAction::Dispatch)
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
    kind: BlockKind,
    line_no: usize,
    condition: Condition,
    imports: &mut BTreeSet<HostImport>,
) -> Result<()> {
    match condition {
        Condition::OutputBlocked => {
            ensure_kind(kind, BlockKind::Drill, line_no, "output_blocked")?;
            imports.insert(HostImport::DrillOutputBlocked);
        }
        Condition::OutputAvailable(_) => {
            ensure_kind(kind, BlockKind::Router, line_no, "output_available")?;
            imports.insert(HostImport::RouterOutputAvailable);
        }
        Condition::CanProduce => {
            ensure_kind(kind, BlockKind::Assembler, line_no, "can_produce")?;
            imports.insert(HostImport::AssemblerCanProduce);
        }
        Condition::AmmoGtZero => {
            ensure_kind(kind, BlockKind::Turret, line_no, "ammo_count")?;
            imports.insert(HostImport::TurretAmmoCount);
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

fn ensure_kind(kind: BlockKind, expected: BlockKind, line_no: usize, api: &str) -> Result<()> {
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
            out.push(format!("    (if {}", render_condition(*condition)));
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
        Condition::CanProduce => "(call $can_produce)".to_string(),
        Condition::AmmoGtZero => "(i32.gt_s (call $ammo_count) (i32.const 0))".to_string(),
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
