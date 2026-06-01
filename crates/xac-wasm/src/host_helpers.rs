use wasmtime::{Caller, Extern};
use xac_core::{Direction, ItemKind};

use super::abi_codes::direction_index;
use super::{
    AssemblerCommand, BehaviorHostState, BehaviorIntent, DrillCommand, DronePortCommand,
    NetStoreDelete, NetStoreOp, NetStoreWrite,
};

pub(super) fn turret_can_attack_scan_index(state: &BehaviorHostState, index: i32) -> bool {
    state.input.ammo_count > 0 && index >= 0 && index < state.input.turret_visible_enemy_count
}

pub(super) fn drone_loadable_amount(
    state: &BehaviorHostState,
    item: &ItemKind,
    requested: u32,
) -> u32 {
    if !state.input.drone_can_work {
        return 0;
    }
    let cargo_free = u32::try_from(state.input.drone_cargo_free.max(0)).unwrap_or(u32::MAX);
    let available = state
        .input
        .drone_contact_inventory_counts
        .get(item)
        .copied()
        .unwrap_or(0)
        .max(0);
    let available = u32::try_from(available).unwrap_or(u32::MAX);
    requested.min(cargo_free).min(available)
}

pub(super) fn drone_unloadable_amount(
    state: &BehaviorHostState,
    item: &ItemKind,
    requested: u32,
) -> u32 {
    if !state.input.drone_can_work {
        return 0;
    }
    let cargo_count = state
        .input
        .drone_cargo_counts
        .get(item)
        .copied()
        .unwrap_or(0)
        .max(0);
    let cargo_count = u32::try_from(cargo_count).unwrap_or(u32::MAX);
    let contact_space = state
        .input
        .drone_contact_space_counts
        .get(item)
        .copied()
        .unwrap_or(0)
        .max(0);
    let contact_space = u32::try_from(contact_space).unwrap_or(u32::MAX);
    requested.min(cargo_count).min(contact_space)
}

pub(super) fn router_any_output_available(state: &BehaviorHostState) -> bool {
    state
        .input
        .router_output_available
        .iter()
        .any(|available| *available)
}

pub(super) fn router_output_available(state: &BehaviorHostState, dir: Direction) -> bool {
    state.input.router_output_available[direction_index(dir)]
}

pub(super) fn router_item_output_available(
    state: &BehaviorHostState,
    item: &ItemKind,
    dir: Direction,
) -> bool {
    state
        .input
        .router_item_output_available
        .get(item)
        .map(|by_dir| by_dir[direction_index(dir)])
        .unwrap_or(false)
}

pub(super) fn push_drill_command(state: &mut BehaviorHostState, command: DrillCommand) {
    match &mut state.intent {
        BehaviorIntent::Drill { commands } => commands.push(command),
        _ => {
            state.intent = BehaviorIntent::Drill {
                commands: vec![command],
            };
        }
    }
}

pub(super) fn push_assembler_command(state: &mut BehaviorHostState, command: AssemblerCommand) {
    match &mut state.intent {
        BehaviorIntent::Assembler { commands } => commands.push(command),
        _ => {
            state.intent = BehaviorIntent::Assembler {
                commands: vec![command],
            };
        }
    }
}

pub(super) fn push_drone_port_command(state: &mut BehaviorHostState, command: DronePortCommand) {
    match &mut state.intent {
        BehaviorIntent::DronePort { commands } => commands.push(command),
        _ => {
            state.intent = BehaviorIntent::DronePort {
                commands: vec![command],
            };
        }
    }
}

pub(super) fn push_net_store_set(state: &mut BehaviorHostState, key: i32, value: i32) {
    state.input.net_i32.insert(key, value);
    state
        .net_ops
        .push(NetStoreOp::Set(NetStoreWrite { key, value }));
}

pub(super) fn push_net_store_delete(state: &mut BehaviorHostState, key: i32) {
    state.input.net_i32.remove(&key);
    state
        .net_ops
        .push(NetStoreOp::Delete(NetStoreDelete { key }));
}

pub(super) fn read_guest_string(
    caller: &mut Caller<'_, BehaviorHostState>,
    ptr: usize,
    len: usize,
) -> Option<String> {
    let memory = match caller.get_export("memory")? {
        Extern::Memory(memory) => memory,
        _ => return None,
    };
    let mut bytes = vec![0_u8; len];
    memory.read(caller, ptr, &mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}

pub(super) fn charge_host(caller: &mut Caller<'_, BehaviorHostState>, cost: u64) -> bool {
    let Ok(fuel) = caller.get_fuel() else {
        caller.data_mut().host_over_budget = true;
        return false;
    };
    if fuel < cost {
        caller.data_mut().host_over_budget = true;
        return false;
    }
    if caller.set_fuel(fuel - cost).is_err() {
        caller.data_mut().host_over_budget = true;
        return false;
    }
    true
}
