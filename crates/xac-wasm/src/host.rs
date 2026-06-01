use anyhow::Result;
use wasmtime::{Caller, Linker};
use xac_core::{Direction, Pos};

use super::abi_codes::{
    direction_from_code, dropoff_tag_from_code, enemy_kind_code, item_code, item_from_code,
    recipe_code, recipe_from_code, recipe_index,
};
use super::host_helpers::{
    charge_host, drone_loadable_amount, drone_unloadable_amount, push_assembler_command,
    push_drill_command, push_drone_port_command, push_net_store_delete, push_net_store_set,
    read_guest_string, router_any_output_available, router_item_output_available,
    router_output_available, turret_can_attack_scan_index,
};
use super::{
    attack_policy_to_rules, host_cost, AssemblerCommand, BehaviorHostState, BehaviorIntent,
    BehaviorLog, DrillCommand, DroneCommand, DronePortCommand, TargetRule,
};

const MAX_LOG_MESSAGE_BYTES: usize = 256;

pub(super) fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    linker.func_wrap(
        "xac:common",
        "log",
        |mut caller: Caller<'_, BehaviorHostState>, ptr: i32, len: i32| -> i32 {
            let Ok(len) = usize::try_from(len) else {
                return 0;
            };
            let Ok(ptr) = usize::try_from(ptr) else {
                return 0;
            };
            if len > MAX_LOG_MESSAGE_BYTES {
                return 0;
            }
            let cost = host_cost::LOG_BASE + (len as u64 / 32);
            if !charge_host(&mut caller, cost) {
                return 0;
            }
            let Some(message) = read_guest_string(&mut caller, ptr, len) else {
                return 0;
            };
            caller.data_mut().logs.push(BehaviorLog { message });
            1
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "fuel_remaining",
        |mut caller: Caller<'_, BehaviorHostState>| -> i64 {
            charge_host(&mut caller, host_cost::FUEL_REMAINING);
            caller.get_fuel().unwrap_or(0) as i64
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "inventory_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::INVENTORY_COUNT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .inventory_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "inventory_free",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::INVENTORY_FREE) {
                return 0;
            }
            caller.data().input.inventory_free
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "stock_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::STOCK_COUNT) {
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
        "xac:common",
        "stock_capacity",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::STOCK_CAPACITY) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .network_stock_capacity
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "has_space",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, amount: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::HAS_SPACE) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 1;
            }
            let space = caller
                .data()
                .input
                .network_stock_space
                .get(&item)
                .copied()
                .unwrap_or(0);
            if space >= i32::try_from(amount).unwrap_or(i32::MAX) {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:drill",
        "output_blocked",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::OUTPUT_BLOCKED) {
                return 0;
            }
            if caller.data().input.output_blocked {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:drill",
        "mine",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::MINE) {
                return 0;
            }
            if !caller.data().input.drill_can_mine {
                return 0;
            }
            push_drill_command(caller.data_mut(), DrillCommand::Mine);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drill",
        "output",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRILL_OUTPUT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            if !caller
                .data()
                .input
                .drill_output_available
                .get(&item)
                .copied()
                .unwrap_or(false)
            {
                return 0;
            }
            push_drill_command(caller.data_mut(), DrillCommand::Output { item });
            1
        },
    )?;
    linker.func_wrap(
        "xac:drill",
        "ore_kind",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::ORE_KIND) {
                return -1;
            }
            caller
                .data()
                .input
                .drill_ore_kind
                .as_ref()
                .map(item_code)
                .unwrap_or(-1)
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_any",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::PUSH) {
                return 0;
            }
            if !router_any_output_available(caller.data()) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::Router {
                item: None,
                preferred: Direction::all().to_vec(),
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_dir",
        |mut caller: Caller<'_, BehaviorHostState>, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::PUSH) {
                return 0;
            }
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            if !router_output_available(caller.data(), dir) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::Router {
                item: None,
                preferred: vec![dir],
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_item_dir",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::PUSH_ITEM) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            if !router_item_output_available(caller.data(), &item, dir) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::Router {
                item: Some(item),
                preferred: vec![dir],
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "output_available",
        |mut caller: Caller<'_, BehaviorHostState>, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::OUTPUT_AVAILABLE) {
                return 0;
            }
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            if router_output_available(caller.data(), dir) {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "output_item_available",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::OUTPUT_ITEM_AVAILABLE) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            if router_item_output_available(caller.data(), &item, dir) {
                1
            } else {
                0
            }
        },
    )?;
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
    linker.func_wrap(
        "xac:net",
        "store_get_i32",
        |mut caller: Caller<'_, BehaviorHostState>, key: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::NET_GET_I32) {
                return 0;
            }
            caller.data().input.net_i32.get(&key).copied().unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:net",
        "store_set_i32",
        |mut caller: Caller<'_, BehaviorHostState>, key: i32, value: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::NET_SET_I32) {
                return 0;
            }
            let data = caller.data_mut();
            if !data.input.net_writable {
                return 0;
            }
            push_net_store_set(data, key, value);
            1
        },
    )?;
    linker.func_wrap(
        "xac:net",
        "store_delete_i32",
        |mut caller: Caller<'_, BehaviorHostState>, key: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::NET_DELETE_I32) {
                return 0;
            }
            let data = caller.data_mut();
            if !data.input.net_writable {
                return 0;
            }
            push_net_store_delete(data, key);
            1
        },
    )?;
    Ok(())
}
