use super::*;
use crate::abi_codes::direction_index;
use std::collections::BTreeMap;

fn item_counts(item: ItemKind, amount: i32) -> BTreeMap<ItemKind, i32> {
    BTreeMap::from([(item, amount)])
}

fn drill_can_mine_input() -> BehaviorHostInput {
    BehaviorHostInput {
        drill_can_mine: true,
        ..Default::default()
    }
}

fn drill_output_input(item: ItemKind) -> BehaviorHostInput {
    BehaviorHostInput {
        drill_ore_kind: Some(item.clone()),
        drill_output_available: BTreeMap::from([(item, true)]),
        ..Default::default()
    }
}

fn router_item_available(item: ItemKind, dir: Direction) -> BTreeMap<ItemKind, [bool; 4]> {
    let mut by_dir = [false; 4];
    by_dir[direction_index(dir)] = true;
    BTreeMap::from([(item, by_dir)])
}

#[test]
fn wit_draft_declares_current_behavior_abi() {
    let wit = include_str!("../../../assets/wit/xac.mvp.wit");
    let mut resolve = wit_parser::Resolve::default();
    resolve
        .push_str("assets/wit/xac.mvp.wit", wit)
        .expect("WIT draft should parse");

    let mut seen_raw_imports = std::collections::BTreeSet::new();
    for spec in HOST_IMPORT_SPECS {
        assert!(
            seen_raw_imports.insert((spec.module, spec.name)),
            "duplicate host import spec for {}/{}",
            spec.module,
            spec.name
        );
        for kind in ALL_BEHAVIOR_KINDS {
            assert_eq!(
                allowed_host_import(*kind, spec.module, spec.name),
                spec.allowed_for(*kind),
                "{kind:?} import allowance should match spec for {}/{}",
                spec.module,
                spec.name
            );
        }
        assert!(
            wit.contains(spec.wit_name),
            "WIT should declare {}",
            spec.wit_name
        );
    }

    for world in [
        "drill-behavior",
        "router-behavior",
        "assembler-behavior",
        "turret-behavior",
        "drone-port-behavior",
        "carrier-drone-behavior",
    ] {
        let world_header = format!("world {world}");
        let start = wit.find(&world_header).expect("world should exist in WIT");
        let rest = &wit[start + world_header.len()..];
        let end = rest.find("\nworld ").unwrap_or(rest.len());
        let world_body = &rest[..end];
        assert!(
            world_body.contains("import net-api;"),
            "{world} should import the shared network store API"
        );
        assert!(
            world_body.contains("export tick: func();"),
            "{world} should use the runtime tick ABI"
        );
    }
    assert!(
        !wit.contains("on-item"),
        "router WIT should not describe the removed on-item export ABI"
    );
}

#[test]
fn compiles_wat_and_evaluates_action_code() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Drill, &wat_const_action(1))
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 20, BehaviorHostInput::default())
        .unwrap();

    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));
    assert!(!eval.over_budget);
    assert!(eval.fuel_spent > 0);
    assert_eq!(compiled.wasm_hash(), eval.wasm_hash);
}

#[test]
fn rejects_invalid_wat_and_missing_tick() {
    let runtime = BehaviorRuntime::new().unwrap();

    assert!(runtime.compile_wat(BehaviorKind::Drill, "not wat").is_err());
    assert!(runtime
        .compile_wat(BehaviorKind::Drill, "(module)")
        .is_err());
    assert!(runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"(module (func (export "tick") (result i64) (i64.const 1)))"#
        )
        .is_err());
}

#[test]
fn reports_over_budget_when_fuel_is_exhausted() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Drill, &wat_spin_action(10_000, 1))
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 1, BehaviorHostInput::default())
        .unwrap();

    assert!(eval.over_budget);
    assert!(eval.runtime_error.is_none());
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn reports_invalid_action_code_as_runtime_error() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Drill, &wat_const_action(30))
        .unwrap();

    let eval = runtime
        .evaluate_compiled(&compiled, 20, BehaviorHostInput::default())
        .unwrap();
    assert!(!eval.over_budget);
    assert!(eval
        .runtime_error
        .as_deref()
        .unwrap_or_default()
        .contains("invalid action code 30"));
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn reports_wasm_trap_as_runtime_error_not_over_budget() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"(module (func (export "tick") unreachable))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(&compiled, 20, BehaviorHostInput::default())
        .unwrap();
    assert!(!eval.over_budget);
    assert!(eval.fuel_spent > 0);
    assert!(eval
        .runtime_error
        .as_deref()
        .unwrap_or_default()
        .contains("UnreachableCodeReached"));
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn maps_router_and_assembler_actions() {
    assert!(matches!(
        action_code_to_intent(BehaviorKind::Router, 12).unwrap(),
        BehaviorIntent::Router { item, preferred }
            if item.is_none() && preferred == vec![Direction::East]
    ));
    assert!(matches!(
        action_code_to_intent(BehaviorKind::Assembler, 21).unwrap(),
        BehaviorIntent::Assembler { ref commands }
            if commands == &vec![
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Ammo
                },
                AssemblerCommand::Produce {
                    recipe: ItemKind::Ammo
                }
            ]
    ));
}

#[test]
fn host_imports_allow_drill_code_to_call_game_api() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Drill, &wat_drill_mine())
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 30, drill_can_mine_input())
        .unwrap();

    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));
    assert!(!eval.over_budget);

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            30,
            BehaviorHostInput {
                output_blocked: true,
                drill_can_mine: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn drill_wat_can_query_ore_kind_and_output_item() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"(module
                  (import "xac:drill" "ore_kind" (func $ore_kind (result i32)))
                  (import "xac:drill" "output" (func $output (param i32) (result i32)))
                  (import "xac:drill" "mine" (func $mine (result i32)))
                  (func (export "tick")
                    (if (i32.eq (call $ore_kind) (i32.const 0))
                      (then
                        (drop (call $output (i32.const 0))))
                      (else
                        (drop (call $mine))))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(&compiled, 40, drill_output_input(ItemKind::Ore))
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Output { item: ItemKind::Ore }]
    ));

    let eval = runtime
        .evaluate_compiled(&compiled, 40, drill_can_mine_input())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));
}

#[test]
fn drill_physical_imports_return_availability() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled_mine = runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"(module
                  (import "xac:drill" "mine" (func $mine (result i32)))
                  (import "xac:net" "store_set_i32" (func $net_set (param i32 i32) (result i32)))
                  (func (export "tick")
                    (drop (call $net_set (i32.const 1) (call $mine)))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled_mine,
            40,
            BehaviorHostInput {
                drill_can_mine: true,
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 1, value: 1 })]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands } if commands == &vec![DrillCommand::Mine]
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled_mine,
            40,
            BehaviorHostInput {
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 1, value: 0 })]
    );
    assert!(matches!(eval.intent, BehaviorIntent::Noop));

    let compiled_output = runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"(module
                  (import "xac:drill" "output" (func $output (param i32) (result i32)))
                  (import "xac:net" "store_set_i32" (func $net_set (param i32 i32) (result i32)))
                  (func (export "tick")
                    (drop (call $net_set (i32.const 2) (call $output (i32.const 0))))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled_output,
            40,
            BehaviorHostInput {
                drill_output_available: BTreeMap::from([(ItemKind::Ore, true)]),
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 2, value: 1 })]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Output { item: ItemKind::Ore }]
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled_output,
            40,
            BehaviorHostInput {
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 2, value: 0 })]
    );
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn raw_wat_cannot_import_another_block_capability() {
    let runtime = BehaviorRuntime::new().unwrap();
    let err = match runtime.compile_wat(
        BehaviorKind::Drill,
        r#"(module
                  (import "xac:turret" "attack_nearest" (func $attack_nearest (result i32)))
                  (func (export "tick")
                    (drop (call $attack_nearest))))"#,
    ) {
        Ok(_) => panic!("drill WAT should not import turret host APIs"),
        Err(error) => error,
    };

    assert!(err
        .to_string()
        .contains("Drill behavior cannot import xac:turret/attack_nearest"));
}

#[test]
fn raw_wat_cannot_import_wasi_or_other_external_hosts() {
    let runtime = BehaviorRuntime::new().unwrap();
    let err = match runtime.compile_wat(
            BehaviorKind::Router,
            r#"(module
                  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
                  (func (export "tick")
                    nop))"#,
        ) {
            Ok(_) => panic!("router WAT should not import WASI"),
            Err(error) => error,
        };

    assert!(err
        .to_string()
        .contains("Router behavior cannot import wasi_snapshot_preview1/fd_write"));
}

#[test]
fn compiles_xac_script_to_host_imported_wasm() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
            # short player-facing drill code
            if output_blocked return
            mine
        "#;
    let wat = compile_source_to_wat(BehaviorKind::Drill, source).unwrap();
    assert!(wat.contains(r#"(import "xac:drill" "mine""#));
    assert!(wat.contains("(call $output_blocked)"));

    let compiled = runtime.compile_wat(BehaviorKind::Drill, source).unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 30, drill_can_mine_input())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            30,
            BehaviorHostInput {
                output_blocked: true,
                drill_can_mine: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn compiles_tiny_source_to_host_imported_wasm() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
            fn tick() {
              if (output_blocked()) { return; }
              mine();
            }
        "#;
    let wat = compile_source_to_wat(BehaviorKind::Drill, source).unwrap();
    assert!(wat.contains(r#"(import "xac:drill" "mine""#));
    assert!(wat.contains("(call $output_blocked)"));

    let compiled = runtime.compile_wat(BehaviorKind::Drill, source).unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 40, drill_can_mine_input())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                output_blocked: true,
                drill_can_mine: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn tiny_source_uses_same_fuel_budget_as_wasm_behaviors() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"
                fn tick() {
                  if (fuel_remaining() > 12) { mine(); }
                }
                "#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(&compiled, 8, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
    assert!(!eval.over_budget);

    let eval = runtime
        .evaluate_compiled(&compiled, 40, drill_can_mine_input())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));
    assert!(eval.fuel_spent >= host_cost::MINE);
}

#[test]
fn xac_script_can_use_drill_output_and_ore_kind() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "if ore_kind == ore output ore";
    let wat = compile_source_to_wat(BehaviorKind::Drill, source).unwrap();
    assert!(wat.contains(r#"(import "xac:drill" "ore_kind""#));
    assert!(wat.contains(r#"(import "xac:drill" "output""#));

    let compiled = runtime.compile_wat(BehaviorKind::Drill, source).unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 40, drill_output_input(ItemKind::Ore))
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Output { item: ItemKind::Ore }]
    ));
}

#[test]
fn xac_script_can_read_and_write_network_store() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Turret,
            r#"
                  net_set 7 42
                  if net 7 == 42 attack_best lowest_hp
                "#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                ammo_count: 3,
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 7, value: 42 })]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Turret { priority } if matches!(
            priority.as_slice(),
            [TargetRule::LowestHp, TargetRule::Nearest]
        )
    ));
}

#[test]
fn xac_script_can_delete_network_store_key() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
        net_delete 7
        if net 7 == 0 attack_nearest
    "#;
    let wat = compile_source_to_wat(BehaviorKind::Turret, source).unwrap();
    assert!(wat.contains(r#""store_delete_i32""#));

    let compiled = runtime.compile_wat(BehaviorKind::Turret, source).unwrap();
    let mut net_i32 = BTreeMap::new();
    net_i32.insert(7, 42);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                ammo_count: 3,
                net_i32,
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Delete(NetStoreDelete { key: 7 })]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Turret { ref priority } if matches!(priority.as_slice(), [TargetRule::Nearest])
    ));
}

#[test]
fn tiny_source_can_delete_network_store_key() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
        fn tick() {
            net_set(7, 42);
            net_delete(7);
        }
    "#;
    let wat = compile_source_to_wat(BehaviorKind::Router, source).unwrap();
    assert!(wat.contains(r#""store_delete_i32""#));

    let compiled = runtime.compile_wat(BehaviorKind::Router, source).unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(
        eval.net_ops,
        vec![
            NetStoreOp::Set(NetStoreWrite { key: 7, value: 42 }),
            NetStoreOp::Delete(NetStoreDelete { key: 7 })
        ]
    );
}

#[test]
fn xac_script_attack_best_accepts_enemy_kind_priority() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Turret,
            "if ammo_count > 0 attack_best runner wire_cutter armored nearest",
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                ammo_count: 3,
                ..Default::default()
            },
        )
        .unwrap();

    assert!(matches!(
        eval.intent,
        BehaviorIntent::Turret { priority } if matches!(
            priority.as_slice(),
            [
                TargetRule::Kind(EnemyKind::Runner),
                TargetRule::Kind(EnemyKind::WireCutter),
                TargetRule::Kind(EnemyKind::Armored),
                TargetRule::Nearest
            ]
        )
    ));
}

#[test]
fn turret_wat_can_scan_and_attack_by_index() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Turret,
            r#"(module
                  (import "xac:turret" "scan_enemies" (func $scan_enemies (result i32)))
                  (import "xac:turret" "can_attack" (func $can_attack (param i32) (result i32)))
                  (import "xac:turret" "attack" (func $attack (param i32) (result i32)))
                  (func (export "tick")
                    (if (i32.gt_s (call $scan_enemies) (i32.const 1))
                      (then
                        (if (call $can_attack (i32.const 1))
                          (then
                            (drop (call $attack (i32.const 1)))))))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                ammo_count: 3,
                turret_visible_enemy_count: 2,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::TurretScanIndex { index } if index == 1
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                ammo_count: 3,
                turret_visible_enemy_count: 1,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn xac_script_can_scan_and_attack_by_index() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "if scan_enemies > 1 attack 1";
    let wat = compile_source_to_wat(BehaviorKind::Turret, source).unwrap();
    assert!(wat.contains(r#""scan_enemies""#));
    assert!(wat.contains(r#""attack""#));

    let compiled = runtime.compile_wat(BehaviorKind::Turret, source).unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                ammo_count: 3,
                turret_visible_enemy_count: 2,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::TurretScanIndex { index } if index == 1
    ));
}

#[test]
fn xac_script_can_branch_on_scanned_enemy_info() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "\
if enemy_kind 1 == runner attack 1
if enemy_hp 0 < 15 attack 0
if enemy_distance 0 < 2.5 attack 0";
    let wat = compile_source_to_wat(BehaviorKind::Turret, source).unwrap();
    assert!(wat.contains(r#""enemy_kind""#));
    assert!(wat.contains(r#""enemy_hp""#));
    assert!(wat.contains(r#""enemy_distance""#));

    let compiled = runtime.compile_wat(BehaviorKind::Turret, source).unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                ammo_count: 3,
                turret_visible_enemy_count: 2,
                turret_visible_enemy_kinds: vec![EnemyKind::Grunt, EnemyKind::Runner],
                turret_visible_enemy_hp: vec![30, 20],
                turret_visible_enemy_distance: vec![3.0, 4.0],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::TurretScanIndex { index } if index == 1
    ));
}

#[test]
fn tiny_source_can_branch_on_scanned_enemy_info() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
        fn tick() {
            if (enemy_kind(1) == runner) { attack(1); }
            if (enemy_hp(0) < 15) { attack(0); }
            if (enemy_distance(0) < 2.5) { attack(0); }
        }
    "#;
    let wat = compile_source_to_wat(BehaviorKind::Turret, source).unwrap();
    assert!(wat.contains(r#""enemy_kind""#));
    assert!(wat.contains(r#""enemy_hp""#));
    assert!(wat.contains(r#""enemy_distance""#));

    let compiled = runtime.compile_wat(BehaviorKind::Turret, source).unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                ammo_count: 3,
                turret_visible_enemy_count: 2,
                turret_visible_enemy_kinds: vec![EnemyKind::Grunt, EnemyKind::Runner],
                turret_visible_enemy_hp: vec![30, 20],
                turret_visible_enemy_distance: vec![3.0, 4.0],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::TurretScanIndex { index } if index == 1
    ));
}

#[test]
fn xac_script_can_gate_router_push_on_output_availability() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Router, "if output_available east push east")
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            30,
            BehaviorHostInput {
                router_output_available: [false, true, false, false],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item.is_none() && preferred == vec![Direction::East]
    ));
}

#[test]
fn xac_script_can_push_specific_router_item() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "if output_available ammo east push ammo east";
    let wat = compile_source_to_wat(BehaviorKind::Router, source).unwrap();
    assert!(wat.contains(r#""output_item_available""#));
    assert!(wat.contains(r#""push_item_dir""#));

    let compiled = runtime.compile_wat(BehaviorKind::Router, source).unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 80, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));

    let mut by_item = BTreeMap::new();
    by_item.insert(ItemKind::Ammo, [false, true, false, false]);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                router_item_output_available: by_item,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ammo) && preferred == vec![Direction::East]
    ));
}

#[test]
fn router_push_imports_return_physical_availability() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Router,
            r#"(module
                  (import "xac:router" "push_any" (func $push_any (result i32)))
                  (import "xac:router" "push_dir" (func $push_dir (param i32) (result i32)))
                  (import "xac:router" "push_item_dir" (func $push_item_dir (param i32 i32) (result i32)))
                  (import "xac:net" "store_set_i32" (func $net_set (param i32 i32) (result i32)))
                  (func (export "tick")
                    (drop (call $net_set (i32.const 1) (call $push_any)))
                    (drop (call $net_set (i32.const 2) (call $push_dir (i32.const 1))))
                    (drop (call $net_set (i32.const 3) (call $push_item_dir (i32.const 2) (i32.const 1))))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![
            NetStoreOp::Set(NetStoreWrite { key: 1, value: 0 }),
            NetStoreOp::Set(NetStoreWrite { key: 2, value: 0 }),
            NetStoreOp::Set(NetStoreWrite { key: 3, value: 0 }),
        ]
    );
    assert!(matches!(eval.intent, BehaviorIntent::Noop));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                router_output_available: [false, true, false, false],
                router_item_output_available: router_item_available(
                    ItemKind::Ammo,
                    Direction::East,
                ),
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![
            NetStoreOp::Set(NetStoreWrite { key: 1, value: 1 }),
            NetStoreOp::Set(NetStoreWrite { key: 2, value: 1 }),
            NetStoreOp::Set(NetStoreWrite { key: 3, value: 1 }),
        ]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ammo) && preferred == vec![Direction::East]
    ));
}

#[test]
fn xac_script_can_read_common_network_stock() {
    let runtime = BehaviorRuntime::new().unwrap();
    let stock_script = "if stock_count ammo > 5 push ammo east";
    let wat = compile_source_to_wat(BehaviorKind::Router, stock_script).unwrap();
    assert!(wat.contains(r#""xac:common" "stock_count""#));
    let compiled = runtime
        .compile_wat(BehaviorKind::Router, stock_script)
        .unwrap();
    let mut counts = BTreeMap::new();
    counts.insert(ItemKind::Ammo, 8);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                network_stock_counts: counts,
                router_item_output_available: router_item_available(
                    ItemKind::Ammo,
                    Direction::East,
                ),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ammo) && preferred == vec![Direction::East]
    ));

    let space_script = "if has_space ore 2 push ore east";
    let wat = compile_source_to_wat(BehaviorKind::Router, space_script).unwrap();
    assert!(wat.contains(r#""xac:common" "has_space""#));
    let compiled = runtime
        .compile_wat(BehaviorKind::Router, space_script)
        .unwrap();
    let mut space = BTreeMap::new();
    space.insert(ItemKind::Ore, 3);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                network_stock_space: space,
                router_item_output_available: router_item_available(ItemKind::Ore, Direction::East),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ore) && preferred == vec![Direction::East]
    ));

    let capacity_script = "if stock_capacity ore >= 100 push ore east";
    let wat = compile_source_to_wat(BehaviorKind::Router, capacity_script).unwrap();
    assert!(wat.contains(r#""xac:common" "stock_capacity""#));
    let compiled = runtime
        .compile_wat(BehaviorKind::Router, capacity_script)
        .unwrap();
    let mut capacity = BTreeMap::new();
    capacity.insert(ItemKind::Ore, 120);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                network_stock_capacity: capacity,
                router_item_output_available: router_item_available(ItemKind::Ore, Direction::East),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ore) && preferred == vec![Direction::East]
    ));
}

#[test]
fn xac_script_can_read_local_inventory_profile() {
    let runtime = BehaviorRuntime::new().unwrap();
    let count_script = "if inventory_count ore > 1 push ore east";
    let wat = compile_source_to_wat(BehaviorKind::Router, count_script).unwrap();
    assert!(wat.contains(r#""xac:common" "inventory_count""#));
    let compiled = runtime
        .compile_wat(BehaviorKind::Router, count_script)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                inventory_counts: item_counts(ItemKind::Ore, 2),
                router_item_output_available: router_item_available(ItemKind::Ore, Direction::East),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ore) && preferred == vec![Direction::East]
    ));

    let free_script = "if inventory_free >= 3 push ammo east";
    let wat = compile_source_to_wat(BehaviorKind::Router, free_script).unwrap();
    assert!(wat.contains(r#""xac:common" "inventory_free""#));
    let compiled = runtime
        .compile_wat(BehaviorKind::Router, free_script)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                inventory_free: 3,
                router_item_output_available: router_item_available(
                    ItemKind::Ammo,
                    Direction::East,
                ),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ammo) && preferred == vec![Direction::East]
    ));
}

#[test]
fn tiny_source_can_read_local_inventory_profile() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
        fn tick() {
            if (inventory_count(ore) > 1) {
                push(ore, east);
            }
        }
    "#;
    let wat = compile_source_to_wat(BehaviorKind::Router, source).unwrap();
    assert!(wat.contains(r#""xac:common" "inventory_count""#));
    let compiled = runtime.compile_wat(BehaviorKind::Router, source).unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                inventory_counts: item_counts(ItemKind::Ore, 2),
                router_item_output_available: router_item_available(ItemKind::Ore, Direction::East),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item == Some(ItemKind::Ore) && preferred == vec![Direction::East]
    ));
}

#[test]
fn xac_script_can_select_assembler_recipe_from_inventory_counts() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
            set_recipe plate
            if output_count ammo < 5 set_recipe ammo
            if can_produce produce
        "#;
    let wat = compile_source_to_wat(BehaviorKind::Assembler, source).unwrap();
    assert!(wat.contains(r#""output_count""#));

    let compiled = runtime
        .compile_wat(BehaviorKind::Assembler, source)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                assembler_can_produce: [true, true],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Assembler { ref commands }
            if commands == &vec![
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Plate
                },
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Ammo
                },
                AssemblerCommand::Produce {
                    recipe: ItemKind::Ammo
                }
            ]
    ));

    let mut output_counts = BTreeMap::new();
    output_counts.insert(ItemKind::Ammo, 5);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                assembler_can_produce: [true, true],
                assembler_output_counts: output_counts,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Assembler { ref commands }
            if commands == &vec![
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Plate
                },
                AssemblerCommand::Produce {
                    recipe: ItemKind::Plate
                }
            ]
    ));
}

#[test]
fn assembler_can_read_current_recipe_from_host_state() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Assembler,
            r#"(module
                  (import "xac:assembler" "current_recipe" (func $current_recipe (result i32)))
                  (import "xac:assembler" "set_recipe" (func $set_recipe (param i32) (result i32)))
                  (func (export "tick")
                    (if (i32.eq (call $current_recipe) (i32.const 0))
                      (then
                        (drop (call $set_recipe (i32.const 1)))))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                assembler_current_recipe: Some(ItemKind::Plate),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Assembler { ref commands }
            if commands == &vec![AssemblerCommand::SetRecipe {
                recipe: ItemKind::Ammo
            }]
    ));
}

#[test]
fn xac_script_can_branch_on_current_assembler_recipe() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "if current_recipe == ammo set_recipe plate";
    let wat = compile_source_to_wat(BehaviorKind::Assembler, source).unwrap();
    assert!(wat.contains(r#""current_recipe""#));
    let compiled = runtime
        .compile_wat(BehaviorKind::Assembler, source)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                assembler_current_recipe: Some(ItemKind::Ammo),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Assembler { ref commands }
            if commands == &vec![AssemblerCommand::SetRecipe {
                recipe: ItemKind::Plate
            }]
    ));
}

#[test]
fn xac_script_can_drive_carrier_drone_commands() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = include_str!("../../../assets/builtin/carrier_drone/basic.xac");
    let wat = compile_source_to_wat(BehaviorKind::CarrierDrone, source).unwrap();
    assert!(wat.contains(r#""battery_percent""#));
    assert!(wat.contains(r#""claim_delivery_job""#));
    assert!(wat.contains(r#""deliver""#));

    let compiled = runtime
        .compile_wat(BehaviorKind::CarrierDrone, source)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_battery_percent: 10,
                drone_logic_fuel: 1000,
                drone_has_pending_job: true,
                drone_can_return_to_port: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::ReturnToPort
        }
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_battery_percent: 100,
                drone_logic_fuel: 1000,
                drone_has_job: true,
                drone_can_work: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::Deliver
        }
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_battery_percent: 100,
                drone_logic_fuel: 1000,
                drone_has_pending_job: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::ClaimDeliveryJob
        }
    ));
}

#[test]
fn carrier_drone_wat_can_read_battery_ratio() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::CarrierDrone,
            r#"(module
                  (import "xac:drone" "battery_ratio" (func $battery_ratio (result f32)))
                  (import "xac:drone" "return_to_port" (func $return_to_port (result i32)))
                  (func (export "tick")
                    (if (f32.lt (call $battery_ratio) (f32.const 0.25))
                      (then
                        (drop (call $return_to_port))))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                drone_battery_percent: 20,
                drone_can_return_to_port: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::ReturnToPort
        }
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                drone_battery_percent: 50,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn xac_script_can_branch_on_battery_ratio() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "if battery_ratio < 0.25 return_to_port";
    let wat = compile_source_to_wat(BehaviorKind::CarrierDrone, source).unwrap();
    assert!(wat.contains(r#""battery_ratio""#));
    assert!(wat.contains("f32.lt"));
    let compiled = runtime
        .compile_wat(BehaviorKind::CarrierDrone, source)
        .unwrap();

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                drone_battery_percent: 24,
                drone_can_return_to_port: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::ReturnToPort
        }
    ));
}

#[test]
fn carrier_drone_wat_can_use_low_level_physical_apis() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::CarrierDrone,
            r#"(module
                  (import "xac:drone" "move_to" (func $move_to (param i32 i32) (result i32)))
                  (import "xac:drone" "load" (func $load (param i32 i32) (result i32)))
                  (import "xac:drone" "unload" (func $unload (param i32 i32) (result i32)))
                  (import "xac:drone" "cargo_count" (func $cargo_count (param i32) (result i32)))
                  (func (export "tick")
                    (if (i32.eqz (call $cargo_count (i32.const 2)))
                      (then
                        (drop (call $load (i32.const 2) (i32.const 5)))
                        (return)))
                    (drop (call $move_to (i32.const 42) (i32.const 30)))
                    (drop (call $unload (i32.const 2) (i32.const 5)))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_can_work: true,
                drone_cargo_free: 20,
                drone_contact_inventory_counts: item_counts(ItemKind::Ammo, 5),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::Load {
                item: ItemKind::Ammo,
                amount: 5
            }
        }
    ));

    let mut cargo = BTreeMap::new();
    cargo.insert(ItemKind::Ammo, 5);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_cargo_counts: cargo,
                drone_can_move: true,
                drone_can_work: true,
                drone_contact_space_counts: item_counts(ItemKind::Ammo, 5),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::Unload {
                item: ItemKind::Ammo,
                amount: 5
            }
        }
    ));
}

#[test]
fn drone_physical_imports_return_available_transfer_amounts() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled_load = runtime
        .compile_wat(
            BehaviorKind::CarrierDrone,
            r#"(module
                  (import "xac:drone" "load" (func $load (param i32 i32) (result i32)))
                  (import "xac:net" "store_set_i32" (func $net_set (param i32 i32) (result i32)))
                  (func (export "tick")
                    (drop (call $net_set (i32.const 1) (call $load (i32.const 2) (i32.const 5))))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled_load,
            80,
            BehaviorHostInput {
                drone_can_work: true,
                drone_cargo_free: 3,
                drone_contact_inventory_counts: item_counts(ItemKind::Ammo, 5),
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 1, value: 3 })]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::Load {
                item: ItemKind::Ammo,
                amount: 5
            }
        }
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled_load,
            80,
            BehaviorHostInput {
                drone_cargo_free: 3,
                drone_contact_inventory_counts: item_counts(ItemKind::Ammo, 5),
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 1, value: 0 })]
    );
    assert!(matches!(eval.intent, BehaviorIntent::Noop));

    let compiled_unload = runtime
        .compile_wat(
            BehaviorKind::CarrierDrone,
            r#"(module
                  (import "xac:drone" "unload" (func $unload (param i32 i32) (result i32)))
                  (import "xac:net" "store_set_i32" (func $net_set (param i32 i32) (result i32)))
                  (func (export "tick")
                    (drop (call $net_set (i32.const 2) (call $unload (i32.const 2) (i32.const 5))))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled_unload,
            80,
            BehaviorHostInput {
                drone_can_work: true,
                drone_cargo_counts: item_counts(ItemKind::Ammo, 5),
                drone_contact_space_counts: item_counts(ItemKind::Ammo, 2),
                net_writable: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        eval.net_ops,
        vec![NetStoreOp::Set(NetStoreWrite { key: 2, value: 2 })]
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::Unload {
                item: ItemKind::Ammo,
                amount: 5
            }
        }
    ));
}

#[test]
fn xac_script_can_drive_carrier_drone_low_level_apis() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "if cargo_count ammo == 0 load ammo 5\nif cargo_count ammo > 0 move_to 42 30";
    let wat = compile_source_to_wat(BehaviorKind::CarrierDrone, source).unwrap();
    assert!(wat.contains(r#""cargo_count""#));
    assert!(wat.contains(r#""load""#));
    assert!(wat.contains(r#""move_to""#));

    let compiled = runtime
        .compile_wat(BehaviorKind::CarrierDrone, source)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_can_work: true,
                drone_cargo_free: 20,
                drone_contact_inventory_counts: item_counts(ItemKind::Ammo, 5),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::Load {
                item: ItemKind::Ammo,
                amount: 5
            }
        }
    ));

    let mut cargo = BTreeMap::new();
    cargo.insert(ItemKind::Ammo, 5);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            80,
            BehaviorHostInput {
                drone_cargo_counts: cargo,
                drone_can_move: true,
                ..Default::default()
            },
        )
        .unwrap();
    match eval.intent {
        BehaviorIntent::CarrierDrone {
            command: DroneCommand::MoveTo { pos },
        } => assert_eq!(pos, Pos { x: 42, y: 30 }),
        other => panic!("expected move_to intent, got {other:?}"),
    }
}

#[test]
fn xac_script_can_drive_drone_port_stock_delivery_api() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = include_str!("../../../assets/builtin/drone_port/basic.xac");
    let wat = compile_source_to_wat(BehaviorKind::DronePort, source).unwrap();
    assert!(wat.contains(r#""stock_count""#));
    assert!(wat.contains(r#""create_delivery_job""#));
    assert!(wat.contains(r#""charge_docked_drones""#));
    assert!(wat.contains(r#""dispatch_idle_drones""#));

    let compiled = runtime
        .compile_wat(BehaviorKind::DronePort, source)
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 120, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::DronePort { ref commands }
            if commands == &vec![
                DronePortCommand::ChargeDockedDrones,
                DronePortCommand::DispatchIdleDrones
            ]
    ));

    let mut stock = BTreeMap::new();
    stock.insert(ItemKind::Ammo, 60);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                drone_port_stock_counts: stock,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::DronePort { ref commands }
            if commands == &vec![
                DronePortCommand::ChargeDockedDrones,
                DronePortCommand::CreateDeliveryJob {
                    item: ItemKind::Ammo,
                    amount: 10,
                    dropoff_tag: "frontline".to_string()
                },
                DronePortCommand::DispatchIdleDrones
            ]
    ));
}

#[test]
fn drone_port_script_can_read_docked_and_pending_job_counts() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "\
if docked_drone_count > 0 charge_docked_drones
if pending_job_count == 0 create_delivery_job ammo 10 frontline
if pending_job_count > 0 dispatch_idle_drones";
    let wat = compile_source_to_wat(BehaviorKind::DronePort, source).unwrap();
    assert!(wat.contains(r#""docked_drone_count""#));
    assert!(wat.contains(r#""pending_job_count""#));

    let compiled = runtime
        .compile_wat(BehaviorKind::DronePort, source)
        .unwrap();
    let mut stock = BTreeMap::new();
    stock.insert(ItemKind::Ammo, 60);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                drone_port_stock_counts: stock.clone(),
                drone_port_docked_drone_count: 1,
                drone_port_pending_job_count: 0,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::DronePort { ref commands }
            if commands == &vec![
                DronePortCommand::ChargeDockedDrones,
                DronePortCommand::CreateDeliveryJob {
                    item: ItemKind::Ammo,
                    amount: 10,
                    dropoff_tag: "frontline".to_string()
                },
            ]
    ));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                drone_port_stock_counts: stock,
                drone_port_docked_drone_count: 1,
                drone_port_pending_job_count: 1,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::DronePort { ref commands }
            if commands == &vec![
                DronePortCommand::ChargeDockedDrones,
                DronePortCommand::DispatchIdleDrones
            ]
    ));
}

#[test]
fn tiny_source_can_read_drone_port_counts() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = r#"
        fn tick() {
            if (docked_drone_count() > 0) {
                charge_docked_drones();
            }
            if (pending_job_count() == 0) {
                create_delivery_job(ammo, 10, frontline);
            }
        }
    "#;
    let wat = compile_source_to_wat(BehaviorKind::DronePort, source).unwrap();
    assert!(wat.contains(r#""docked_drone_count""#));
    assert!(wat.contains(r#""pending_job_count""#));

    let compiled = runtime
        .compile_wat(BehaviorKind::DronePort, source)
        .unwrap();
    let mut stock = BTreeMap::new();
    stock.insert(ItemKind::Ammo, 60);
    let eval = runtime
        .evaluate_compiled(
            &compiled,
            120,
            BehaviorHostInput {
                drone_port_stock_counts: stock,
                drone_port_docked_drone_count: 1,
                drone_port_pending_job_count: 0,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::DronePort { ref commands }
            if commands == &vec![
                DronePortCommand::ChargeDockedDrones,
                DronePortCommand::CreateDeliveryJob {
                    item: ItemKind::Ammo,
                    amount: 10,
                    dropoff_tag: "frontline".to_string()
                },
            ]
    ));
}

#[test]
fn xac_script_can_branch_on_remaining_fuel() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Drill, "if fuel_remaining > 12 mine")
        .unwrap();

    let eval = runtime
        .evaluate_compiled(&compiled, 8, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
    assert!(!eval.over_budget);

    let eval = runtime
        .evaluate_compiled(&compiled, 30, drill_can_mine_input())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));
    assert!(
        eval.fuel_spent >= host_cost::MINE,
        "host API mine should charge explicit fuel"
    );
}

#[test]
fn common_log_reads_guest_memory() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Drill,
            r#"(module
                  (import "xac:common" "log" (func $log (param i32 i32) (result i32)))
                  (memory (export "memory") 1)
                  (data (i32.const 8) "ready")
                  (func (export "tick")
                    (drop (call $log (i32.const 8) (i32.const 5)))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(&compiled, 40, BehaviorHostInput::default())
        .unwrap();
    assert_eq!(
        eval.logs,
        vec![BehaviorLog {
            message: "ready".to_string()
        }]
    );
    assert!(
        eval.fuel_spent >= host_cost::LOG_BASE,
        "log should charge explicit host fuel"
    );
}

#[test]
fn xac_script_can_emit_log_message() {
    let runtime = BehaviorRuntime::new().unwrap();
    let source = "log drill ready";
    let wat = compile_source_to_wat(BehaviorKind::Drill, source).unwrap();
    assert!(wat.contains(r#""log""#));
    assert!(wat.contains(r#"(memory (export "memory") 1)"#));

    let compiled = runtime.compile_wat(BehaviorKind::Drill, source).unwrap();
    let eval = runtime
        .evaluate_compiled(&compiled, 40, BehaviorHostInput::default())
        .unwrap();
    assert_eq!(
        eval.logs,
        vec![BehaviorLog {
            message: "drill ready".to_string()
        }]
    );
}

#[test]
fn host_api_cost_can_exhaust_behavior_budget() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(
            BehaviorKind::Turret,
            r#"(module
                  (import "xac:turret" "attack_best" (func $attack_best (param i32) (result i32)))
                  (func (export "tick")
                    (drop (call $attack_best (i32.const 1)))))"#,
        )
        .unwrap();

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            host_cost::ATTACK_BEST - 1,
            BehaviorHostInput {
                ammo_count: 5,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(eval.over_budget);
    assert!(matches!(eval.intent, BehaviorIntent::Noop));

    let eval = runtime
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                ammo_count: 5,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!eval.over_budget);
    assert!(
        eval.fuel_spent >= host_cost::ATTACK_BEST,
        "attack_best should charge explicit host fuel"
    );
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Turret { priority } if matches!(
            priority.as_slice(),
            [TargetRule::LowestHp, TargetRule::Nearest]
        )
    ));

    let crowded_scan = BehaviorHostInput {
        ammo_count: 5,
        turret_visible_enemy_count: 3,
        ..Default::default()
    };
    let eval = runtime
        .evaluate_compiled(&compiled, host_cost::ATTACK_BEST + 2, crowded_scan.clone())
        .unwrap();
    assert!(
        eval.over_budget,
        "attack_best should charge one extra fuel per visible enemy candidate"
    );

    let eval = runtime
        .evaluate_compiled(&compiled, 40, crowded_scan)
        .unwrap();
    assert!(!eval.over_budget);
    assert!(eval.fuel_spent >= host_cost::ATTACK_BEST + 3);
}

#[test]
fn xac_script_rejects_wrong_block_capability() {
    let err = compile_source_to_wat(BehaviorKind::Turret, "mine").unwrap_err();
    assert!(err.to_string().contains("only available to Drill"));
}

#[test]
fn tiny_source_rejects_wrong_block_capability() {
    let err = compile_source_to_wat(BehaviorKind::Turret, "fn tick() { mine(); }").unwrap_err();
    assert!(err.to_string().contains("only available to Drill"));
}

#[test]
fn host_imports_map_logistics_production_and_combat_apis() {
    let runtime = BehaviorRuntime::new().unwrap();

    let router = runtime
        .compile_wat(
            BehaviorKind::Router,
            r#"(module
                  (import "xac:router" "push_dir" (func $push_dir (param i32) (result i32)))
                  (func (export "tick")
                    (drop (call $push_dir (i32.const 1)))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &router,
            30,
            BehaviorHostInput {
                router_output_available: [false, true, false, false],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Router { item, preferred }
            if item.is_none() && preferred == vec![Direction::East]
    ));

    let assembler = runtime
        .compile_wat(
            BehaviorKind::Assembler,
            r#"(module
                  (import "xac:assembler" "set_recipe" (func $set_recipe (param i32) (result i32)))
                  (import "xac:assembler" "produce" (func $produce (result i32)))
                  (func (export "tick")
                    (drop (call $set_recipe (i32.const 1)))
                    (drop (call $produce))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(
            &assembler,
            30,
            BehaviorHostInput {
                assembler_can_produce: [false, true],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Assembler { ref commands }
            if commands == &vec![
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Ammo
                },
                AssemblerCommand::Produce {
                    recipe: ItemKind::Ammo
                }
            ]
    ));

    let turret = runtime
        .compile_wat(
            BehaviorKind::Turret,
            r#"(module
                  (import "xac:turret" "attack_best" (func $attack_best (param i32) (result i32)))
                  (func (export "tick")
                    (drop (call $attack_best (i32.const 1)))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&turret, 30, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
    let eval = runtime
        .evaluate_compiled(
            &turret,
            30,
            BehaviorHostInput {
                ammo_count: 5,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Turret { priority } if matches!(
            priority.as_slice(),
            [TargetRule::LowestHp, TargetRule::Nearest]
        )
    ));

    let drone_port = runtime
        .compile_wat(
            BehaviorKind::DronePort,
            r#"(module
                  (import "xac:drone_port" "dispatch" (func $dispatch (result i32)))
                  (func (export "tick")
                    (drop (call $dispatch))))"#,
        )
        .unwrap();
    let eval = runtime
        .evaluate_compiled(&drone_port, 30, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::DronePort { commands }
            if commands == vec![DronePortCommand::AutoDispatch]
    ));
}
