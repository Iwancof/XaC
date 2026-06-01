use anyhow::Result;
use wasmtime::{Caller, Linker};

use super::abi_codes::enemy_kind_code;
use super::host_helpers::{charge_host, turret_can_attack_scan_index};
use super::{attack_policy_to_rules, host_cost, BehaviorHostState, BehaviorIntent, TargetRule};

pub(super) fn define_turret_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
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
    Ok(())
}
