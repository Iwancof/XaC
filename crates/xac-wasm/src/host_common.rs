use anyhow::Result;
use wasmtime::{Caller, Linker};

use super::abi_codes::item_from_code;
use super::host_helpers::{charge_host, read_guest_string};
use super::{host_cost, BehaviorHostState, BehaviorLog};

const MAX_LOG_MESSAGE_BYTES: usize = 256;

pub(super) fn define_common_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
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
    Ok(())
}
