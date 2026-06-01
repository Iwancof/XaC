use anyhow::Result;
use wasmtime::{Caller, Linker};
use xac_core::Pos;

use super::abi_codes::{
    dropoff_tag_from_code, enemy_kind_code, item_from_code, recipe_code, recipe_from_code,
    recipe_index,
};
use super::host_common::define_common_imports;
use super::host_drill::define_drill_imports;
use super::host_helpers::{
    charge_host, drone_loadable_amount, drone_unloadable_amount, push_assembler_command,
    push_drone_port_command, turret_can_attack_scan_index,
};
use super::host_net::define_net_imports;
use super::host_router::define_router_imports;
use super::{
    attack_policy_to_rules, host_cost, AssemblerCommand, BehaviorHostState, BehaviorIntent,
    DroneCommand, DronePortCommand, TargetRule,
};

pub(super) fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    define_common_imports(linker)?;
    define_drill_imports(linker)?;
    define_router_imports(linker)?;
    linker.func_wrap(
        "xac:assembler",
        "set_recipe",
        |mut caller: Caller<'_, BehaviorHostState>, recipe: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::SET_RECIPE) {
                return 0;
            }
            let Some(recipe) = recipe_from_code(recipe) else {
                return 0;
            };
            caller.data_mut().assembler_recipe = recipe.clone();
            push_assembler_command(caller.data_mut(), AssemblerCommand::SetRecipe { recipe });
            1
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "current_recipe",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::CURRENT_RECIPE) {
                return -1;
            }
            caller
                .data()
                .input
                .assembler_current_recipe
                .as_ref()
                .map(recipe_code)
                .unwrap_or(-1)
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "can_produce",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::CAN_PRODUCE) {
                return 0;
            }
            let recipe = caller.data().assembler_recipe.clone();
            let can_progress = caller.data().input.assembler_can_produce[recipe_index(&recipe)]
                || caller.data().input.can_produce;
            if can_progress {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "input_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ASSEMBLER_COUNT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .assembler_input_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "output_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ASSEMBLER_COUNT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .assembler_output_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "produce",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::PRODUCE) {
                return 0;
            }
            let recipe = caller.data().assembler_recipe.clone();
            let can_progress = caller.data().input.assembler_can_produce[recipe_index(&recipe)]
                || caller.data().input.can_produce;
            if !can_progress {
                return 0;
            }
            push_assembler_command(caller.data_mut(), AssemblerCommand::Produce { recipe });
            1
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "scan_enemies",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            let count = caller.data().input.turret_visible_enemy_count.max(0);
            let cost = host_cost::SCAN_ENEMIES_BASE.saturating_add(count as u64);
            if !charge_host(&mut caller, cost) {
                return 0;
            }
            count
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "enemy_kind",
        |mut caller: Caller<'_, BehaviorHostState>, index: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ENEMY_INFO) {
                return -1;
            }
            let Ok(index) = usize::try_from(index) else {
                return -1;
            };
            caller
                .data()
                .input
                .turret_visible_enemy_kinds
                .get(index)
                .map(enemy_kind_code)
                .unwrap_or(-1)
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "enemy_hp",
        |mut caller: Caller<'_, BehaviorHostState>, index: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ENEMY_INFO) {
                return -1;
            }
            let Ok(index) = usize::try_from(index) else {
                return -1;
            };
            caller
                .data()
                .input
                .turret_visible_enemy_hp
                .get(index)
                .copied()
                .unwrap_or(-1)
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "enemy_distance",
        |mut caller: Caller<'_, BehaviorHostState>, index: i32| -> f32 {
            if !charge_host(&mut caller, host_cost::ENEMY_INFO) {
                return -1.0;
            }
            let Ok(index) = usize::try_from(index) else {
                return -1.0;
            };
            caller
                .data()
                .input
                .turret_visible_enemy_distance
                .get(index)
                .copied()
                .unwrap_or(-1.0)
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "can_attack",
        |mut caller: Caller<'_, BehaviorHostState>, index: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::CAN_ATTACK) {
                return 0;
            }
            if turret_can_attack_scan_index(caller.data(), index) {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "attack",
        |mut caller: Caller<'_, BehaviorHostState>, index: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ATTACK) {
                return 0;
            }
            if !turret_can_attack_scan_index(caller.data(), index) {
                return 0;
            }
            let Ok(index) = u32::try_from(index) else {
                return 0;
            };
            caller.data_mut().intent = BehaviorIntent::TurretScanIndex { index };
            1
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "ammo_count",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::AMMO_COUNT) {
                return 0;
            }
            caller.data().input.ammo_count
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "attack_nearest",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::ATTACK_NEAREST) {
                return 0;
            }
            if caller.data().input.ammo_count <= 0 {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::Turret {
                priority: vec![TargetRule::Nearest],
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "attack_best",
        |mut caller: Caller<'_, BehaviorHostState>, policy: i32| -> i32 {
            let candidate_count = caller.data().input.turret_visible_enemy_count.max(0) as u64;
            let cost = host_cost::ATTACK_BEST.saturating_add(candidate_count);
            if !charge_host(&mut caller, cost) {
                return 0;
            }
            if caller.data().input.ammo_count <= 0 {
                return 0;
            }
            let Some(priority) = attack_policy_to_rules(policy) else {
                return 0;
            };
            caller.data_mut().intent = BehaviorIntent::Turret { priority };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "dispatch",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DISPATCH) {
                return 0;
            }
            push_drone_port_command(caller.data_mut(), DronePortCommand::AutoDispatch);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "stock_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_STOCK) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .network_stock_counts
                .get(&item)
                .or_else(|| caller.data().input.drone_port_stock_counts.get(&item))
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "charge_docked_drones",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_CHARGE) {
                return 0;
            }
            push_drone_port_command(caller.data_mut(), DronePortCommand::ChargeDockedDrones);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "docked_drone_count",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_DOCKED_COUNT) {
                return 0;
            }
            caller.data().input.drone_port_docked_drone_count
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "pending_job_count",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_PENDING_JOB_COUNT) {
                return 0;
            }
            caller.data().input.drone_port_pending_job_count
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "create_delivery_job",
        |mut caller: Caller<'_, BehaviorHostState>,
         item: i32,
         amount: i32,
         dropoff_tag: i32|
         -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_CREATE_JOB) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Some(dropoff_tag) = dropoff_tag_from_code(dropoff_tag) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 0;
            }
            if caller
                .data()
                .input
                .network_stock_counts
                .get(&item)
                .or_else(|| caller.data().input.drone_port_stock_counts.get(&item))
                .copied()
                .unwrap_or(0)
                < i32::try_from(amount).unwrap_or(i32::MAX)
            {
                return 0;
            }
            push_drone_port_command(
                caller.data_mut(),
                DronePortCommand::CreateDeliveryJob {
                    item,
                    amount,
                    dropoff_tag: dropoff_tag.to_string(),
                },
            );
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "dispatch_idle_drones",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_DISPATCH_IDLE) {
                return 0;
            }
            push_drone_port_command(caller.data_mut(), DronePortCommand::DispatchIdleDrones);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "battery_percent",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0;
            }
            caller.data().input.drone_battery_percent
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "battery_ratio",
        |mut caller: Caller<'_, BehaviorHostState>| -> f32 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0.0;
            }
            caller.data().input.drone_battery_percent.clamp(0, 100) as f32 / 100.0
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "logic_fuel_remaining",
        |mut caller: Caller<'_, BehaviorHostState>| -> i64 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0;
            }
            caller.data().input.drone_logic_fuel as i64
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "has_job",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0;
            }
            if caller.data().input.drone_has_job {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "has_pending_job",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_JOB) {
                return 0;
            }
            if caller.data().input.drone_has_pending_job {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "return_to_port",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            if !caller.data().input.drone_can_return_to_port {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::ReturnToPort,
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "claim_delivery_job",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_JOB) {
                return 0;
            }
            if !caller.data().input.drone_has_pending_job {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::ClaimDeliveryJob,
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "deliver",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            if !caller.data().input.drone_has_job {
                return 0;
            }
            if !caller.data().input.drone_can_work {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Deliver,
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "move_to",
        |mut caller: Caller<'_, BehaviorHostState>, x: i32, y: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_MOVE_TO) {
                return 0;
            }
            if !caller.data().input.drone_can_move {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::MoveTo { pos: Pos { x, y } },
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "load",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, amount: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 0;
            }
            let loaded = drone_loadable_amount(caller.data(), &item, amount);
            if loaded == 0 {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Load { item, amount },
            };
            i32::try_from(loaded).unwrap_or(i32::MAX)
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "unload",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, amount: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 0;
            }
            let unloaded = drone_unloadable_amount(caller.data(), &item, amount);
            if unloaded == 0 {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Unload { item, amount },
            };
            i32::try_from(unloaded).unwrap_or(i32::MAX)
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "cargo_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_CARGO) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .drone_cargo_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "idle",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            if !caller.data().input.drone_can_idle {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Idle,
            };
            1
        },
    )?;
    define_net_imports(linker)?;
    Ok(())
}
