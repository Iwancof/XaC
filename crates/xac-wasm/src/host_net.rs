use anyhow::Result;
use wasmtime::{Caller, Linker};

use super::host_helpers::{charge_host, push_net_store_delete, push_net_store_set};
use super::{host_cost, BehaviorHostState};

pub(super) fn define_net_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
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
