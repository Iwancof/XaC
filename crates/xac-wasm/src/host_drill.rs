use anyhow::Result;
use wasmtime::{Caller, Linker};

use super::abi_codes::{item_code, item_from_code};
use super::host_helpers::{charge_host, push_drill_command};
use super::{host_cost, BehaviorHostState, DrillCommand};

pub(super) fn define_drill_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
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
    Ok(())
}
