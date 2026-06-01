use std::collections::BTreeSet;

use xac_core::ItemKind;

use super::abi_codes::{direction_code, enemy_kind_code, item_code, recipe_code};
use super::script_ast::{Condition, CountComparison, LogData, ScriptAction, ScriptStatement};
use super::script_imports::HostImport;

pub(crate) fn render_script_module(
    imports: BTreeSet<HostImport>,
    statements: &[ScriptStatement],
    log_data: &[LogData],
) -> String {
    let mut out = vec!["(module".to_string()];
    for import in imports {
        out.push(import.wat().to_string());
    }
    if !log_data.is_empty() {
        out.push(r#"  (memory (export "memory") 1)"#.to_string());
        for entry in log_data {
            out.push(format!(
                r#"  (data (i32.const {}) "{}")"#,
                entry.offset,
                wat_data_escape(&entry.bytes)
            ));
        }
    }
    out.push(r#"  (func (export "tick")"#.to_string());
    for statement in statements {
        render_statement(statement, &mut out);
    }
    out.push("  )".to_string());
    out.push(")".to_string());
    out.join("\n")
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
    let op = count_comparison_op(comparison);
    format!("({op} (call ${function_name}) (i32.const {value}))")
}

fn render_count_condition(
    function_name: &str,
    item: &ItemKind,
    comparison: CountComparison,
    value: i32,
) -> String {
    let op = count_comparison_op(comparison);
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
    let op = count_comparison_op(comparison);
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

fn count_comparison_op(comparison: CountComparison) -> &'static str {
    match comparison {
        CountComparison::Lt => "i32.lt_s",
        CountComparison::Le => "i32.le_s",
        CountComparison::Eq => "i32.eq",
        CountComparison::Ge => "i32.ge_s",
        CountComparison::Gt => "i32.gt_s",
    }
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
