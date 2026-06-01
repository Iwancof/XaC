use anyhow::Result;
use wasmtime::{Caller, Linker};
use xac_core::Direction;

use super::abi_codes::{direction_from_code, item_from_code};
use super::host_helpers::{
    charge_host, router_any_output_available, router_item_output_available, router_output_available,
};
use super::{host_cost, BehaviorHostState, BehaviorIntent};

pub(super) fn define_router_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
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
    Ok(())
}
