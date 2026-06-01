use super::*;
use std::collections::BTreeMap;

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
    assert!(matches!(eval.intent, BehaviorIntent::Noop));
}

#[test]
fn validates_action_code_against_block_kind() {
    let runtime = BehaviorRuntime::new().unwrap();
    let compiled = runtime
        .compile_wat(BehaviorKind::Drill, &wat_const_action(30))
        .unwrap();

    let err = runtime
        .evaluate_compiled(&compiled, 20, BehaviorHostInput::default())
        .unwrap_err();
    assert!(err.to_string().contains("invalid action code 30"));
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
        .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
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
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                drill_ore_kind: Some(ItemKind::Ore),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Output { item: ItemKind::Ore }]
    ));

    let eval = runtime
        .evaluate_compiled(&compiled, 40, BehaviorHostInput::default())
        .unwrap();
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Drill { ref commands }
            if commands == &vec![DrillCommand::Mine]
    ));
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
        .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
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
        .evaluate_compiled(&compiled, 40, BehaviorHostInput::default())
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
        .evaluate_compiled(&compiled, 40, BehaviorHostInput::default())
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
        .evaluate_compiled(
            &compiled,
            40,
            BehaviorHostInput {
                drill_ore_kind: Some(ItemKind::Ore),
                ..Default::default()
            },
        )
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

    assert_eq!(eval.net_writes, vec![NetStoreWrite { key: 7, value: 42 }]);
    assert!(matches!(
        eval.intent,
        BehaviorIntent::Turret { priority } if matches!(
            priority.as_slice(),
            [TargetRule::LowestHp, TargetRule::Nearest]
        )
    ));
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
        .evaluate_compiled(&compiled, 80, BehaviorHostInput::default())
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
        .evaluate_compiled(&compiled, 80, BehaviorHostInput::default())
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
        .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
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
        .evaluate_compiled(&router, 30, BehaviorHostInput::default())
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
