use anyhow::Result;
use wasmtime::{Caller, Linker};
use xac_core::Pos;

use super::abi_codes::{dropoff_tag_from_code, item_from_code};
use super::host_assembler::define_assembler_imports;
use super::host_common::define_common_imports;
use super::host_drill::define_drill_imports;
use super::host_helpers::{
    charge_host, drone_loadable_amount, drone_unloadable_amount, push_drone_port_command,
};
use super::host_net::define_net_imports;
use super::host_router::define_router_imports;
use super::host_turret::define_turret_imports;
use super::{host_cost, BehaviorHostState, BehaviorIntent, DroneCommand, DronePortCommand};

pub(super) fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    define_common_imports(linker)?;
    define_drill_imports(linker)?;
    define_router_imports(linker)?;
    define_assembler_imports(linker)?;
    define_turret_imports(linker)?;
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
