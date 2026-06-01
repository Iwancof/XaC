use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use xac_core::{BehaviorKind, BehaviorSummary, DroneState, Inventory, WorldPos};

static TEST_CONFIG_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn placing_wire_and_cpu_node_forms_network() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Wire, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    sim.place_block(BlockKind::CpuNode, Pos { x: 35, y: 32 }, Direction::East)
        .unwrap();
    let snapshot = sim.snapshot();
    assert!(snapshot.networks.iter().any(|n| n.cpu_pool >= 200.0));
}

#[test]
fn deconstructing_wire_removes_footprint_and_recomputes_network() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Wire, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let wire_id = sim.selected_id.clone().unwrap();
    sim.place_block(BlockKind::CpuNode, Pos { x: 35, y: 32 }, Direction::East)
        .unwrap();
    assert!(sim.snapshot().networks.iter().any(|n| n.cpu_pool >= 200.0));

    sim.deconstruct_block(&wire_id).unwrap();

    assert!(!sim.blocks.contains_key(&wire_id));
    assert_eq!(sim.block_id_at(Pos { x: 34, y: 32 }), None);
    assert!(
        !sim.snapshot().networks.iter().any(|n| n.cpu_pool >= 200.0),
        "deconstructing the connecting wire should split the cpu node from the core network"
    );
}

#[test]
fn core_cannot_be_deconstructed() {
    let mut sim = test_sim("sim");
    let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
    let err = sim.deconstruct_block(&core_id).unwrap_err();
    assert!(err.to_string().contains("core cannot be deconstructed"));
    assert!(sim.blocks.contains_key(&core_id));
}

#[test]
fn core_cannot_be_placed_after_world_seed() {
    let mut sim = test_sim("sim");
    let err = sim
        .place_block(BlockKind::Core, Pos { x: 10, y: 10 }, Direction::East)
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("core is the initial 4x4 objective"));
    assert_eq!(
        sim.blocks
            .values()
            .filter(|block| block.kind == BlockKind::Core)
            .count(),
        1
    );
}

#[test]
fn core_defeat_stops_simulation() {
    let mut sim = test_sim("sim");
    let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
    sim.blocks.get_mut(&core_id).unwrap().hp = EnemyKind::Grunt.attack_damage();
    sim.set_running(true);

    let enemy_id = sim.make_id("enemy");
    sim.enemies.insert(
        enemy_id.clone(),
        combat::enemy_at(enemy_id, EnemyKind::Grunt, WorldPos { x: 30.5, y: 30.5 }),
    );

    sim.step_ticks(1);

    let snapshot = sim.snapshot();
    assert!(snapshot.status.defeated);
    assert_eq!(snapshot.status.core_hp, 0);
    assert_eq!(snapshot.status.core_max_hp, BlockKind::Core.max_hp());
    assert!(!snapshot.running);
    assert_eq!(sim.blocks[&core_id].status, "core breached");
    assert!(sim
        .logs
        .iter()
        .any(|entry| entry.source == core_id && entry.message.contains("core destroyed")));

    let defeated_tick = sim.tick;
    sim.step_ticks(1);
    assert_eq!(
        sim.tick, defeated_tick,
        "manual ticking should not advance a defeated simulation"
    );
}

#[test]
fn wave_schedule_spawns_mixed_enemy_roles() {
    assert_eq!(wave::current_wave(0), 1);
    assert_eq!(wave::next_wave_in(0), 20);
    assert!(!wave::should_spawn_wave(19));
    assert!(wave::should_spawn_wave(20));
    assert_eq!(wave::wave_enemies(1), vec![EnemyKind::Grunt]);
    assert_eq!(
        wave::wave_enemies(4),
        vec![
            EnemyKind::Grunt,
            EnemyKind::Grunt,
            EnemyKind::Runner,
            EnemyKind::Armored,
            EnemyKind::WireCutter
        ]
    );

    let mut sim = test_sim("sim");
    sim.step_ticks(20);
    assert_eq!(sim.enemies.len(), 1);
    assert_eq!(sim.enemies.values().next().unwrap().kind, EnemyKind::Grunt);

    sim.step_ticks(80);
    let runner_count = sim
        .enemies
        .values()
        .filter(|enemy| enemy.kind == EnemyKind::Runner)
        .count();
    assert_eq!(runner_count, 1);
    assert_eq!(
        sim.logs
            .iter()
            .filter(|entry| entry.source == "wave" && entry.message.contains("contact"))
            .count(),
        2
    );
}

#[test]
fn rotating_conveyor_changes_logistics_direction() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Storage, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let east_storage_id = sim.selected_id.clone().unwrap();
    sim.place_block(BlockKind::Storage, Pos { x: 34, y: 31 }, Direction::East)
        .unwrap();
    let south_storage_id = sim.selected_id.clone().unwrap();
    sim.place_block(BlockKind::Conveyor, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let conveyor_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&conveyor_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ore, 1);

    sim.rotate_block(&conveyor_id).unwrap();
    assert_eq!(sim.blocks[&conveyor_id].dir, Direction::South);
    sim.step_ticks(1);

    assert_eq!(
        sim.blocks[&east_storage_id].inventory.count(&ItemKind::Ore),
        0
    );
    assert_eq!(
        sim.blocks[&south_storage_id]
            .inventory
            .count(&ItemKind::Ore),
        1
    );
}

#[test]
fn core_occupies_four_by_four_tiles() {
    let sim = test_sim("sim");
    let core = sim
        .blocks
        .values()
        .find(|block| block.kind == BlockKind::Core)
        .unwrap();
    assert_eq!(core.pos, Pos { x: 30, y: 30 });
    assert_eq!(sim.block_id_at(Pos { x: 30, y: 30 }), Some(core.id.clone()));
    assert_eq!(sim.block_id_at(Pos { x: 33, y: 33 }), Some(core.id.clone()));
    assert_eq!(sim.block_id_at(Pos { x: 34, y: 33 }), None);
}

#[test]
fn enemy_moves_toward_nearest_point_on_four_by_four_core() {
    let mut sim = test_sim("sim");
    let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
    let enemy_id = sim.make_id("enemy");
    let mut enemy = combat::enemy_at(
        enemy_id.clone(),
        EnemyKind::Grunt,
        WorldPos { x: 32.5, y: 20.5 },
    );
    enemy.move_speed = 1.0;
    sim.enemies.insert(enemy_id.clone(), enemy);

    sim.step_ticks(1);

    let moved = &sim.enemies[&enemy_id];
    assert_eq!(moved.target_id.as_deref(), Some(core_id.as_str()));
    assert_eq!(
            moved.pos.x, 32.5,
            "enemy should move straight toward the closest point on the 4x4 core, not the top-left tile"
        );
    assert!(
        moved.pos.y > 20.5,
        "enemy should advance with non-grid world coordinates"
    );
}

#[test]
fn network_cpu_is_shared_across_active_devices() {
    let mut sim = test_sim("sim");

    sim.place_block(BlockKind::Drill, Pos { x: 29, y: 30 }, Direction::East)
        .unwrap();
    let drill_id = sim.selected_id.clone().unwrap();
    let initial_network_id = sim.blocks[&drill_id].network_id.unwrap();
    let initial_network = sim.networks[&initial_network_id].clone();
    let initial_rate = sim.blocks[&drill_id].effective_cpu_rate;

    assert_eq!(
        initial_network.cpu_pool,
        BlockKind::Core.network_cpu_output()
    );
    assert_eq!(initial_network.active_devices, 1);
    assert_eq!(initial_network.effective_per_device, 120.0);
    assert_eq!(
        initial_rate,
        BlockKind::Drill.local_cpu_rate() + initial_network.effective_per_device
    );

    sim.place_block(BlockKind::Router, Pos { x: 29, y: 31 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    let shared_network_id = sim.blocks[&drill_id].network_id.unwrap();
    let shared_network = sim.networks[&shared_network_id].clone();
    let shared_drill_rate = sim.blocks[&drill_id].effective_cpu_rate;

    assert_eq!(shared_network.active_devices, 2);
    assert_eq!(shared_network.effective_per_device, 60.0);
    assert!(
        shared_drill_rate < initial_rate,
        "adding another programmable device should thin the shared CPU rate"
    );
    assert_eq!(
        sim.blocks[&router_id].effective_cpu_rate,
        BlockKind::Router.local_cpu_rate() + shared_network.effective_per_device
    );

    sim.place_block(BlockKind::CpuNode, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let boosted_network_id = sim.blocks[&drill_id].network_id.unwrap();
    let boosted_network = sim.networks[&boosted_network_id].clone();

    assert_eq!(
        boosted_network.cpu_pool,
        BlockKind::Core.network_cpu_output() + BlockKind::CpuNode.network_cpu_output()
    );
    assert_eq!(boosted_network.active_devices, 2);
    assert_eq!(boosted_network.effective_per_device, 100.0);
    assert!(
        sim.blocks[&drill_id].effective_cpu_rate > shared_drill_rate,
        "adding cpu_node should increase the share without becoming an active device"
    );
}

#[test]
fn builtin_copy_is_editable_and_reassigned() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let block_id = sim.selected_id.clone().unwrap();
    let source = sim.edit_builtin_copy(&block_id).unwrap();
    assert!(!source.summary.builtin);
    assert_eq!(source.summary.used_by, 1);
}

#[test]
fn project_behavior_source_persists_under_config_root() {
    let config_root = test_config_root("behavior-persistence");
    let mut sim = Simulation::new(&config_root).unwrap();
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let block_id = sim.selected_id.clone().unwrap();

    let copied = sim.edit_builtin_copy(&block_id).unwrap();
    let behavior_id = copied.summary.id.clone();
    let source_path = PathBuf::from(&copied.summary.source_path);
    assert!(source_path.starts_with(&config_root));
    assert!(
        fs::read_to_string(&source_path).unwrap().contains("mine"),
        "copy-on-write should create a real source file"
    );

    let edited_source = "if output_blocked return\nmine\nnet_set 9 3";
    sim.save_behavior(&behavior_id, edited_source.to_string())
        .unwrap();
    assert_eq!(fs::read_to_string(&source_path).unwrap(), edited_source);

    let result = sim.build_behavior(&behavior_id).unwrap();
    assert!(result.success);

    let index_source =
        fs::read_to_string(config_root.join("projects/default_project/behaviors.toml")).unwrap();
    assert!(index_source.contains(&behavior_id));
    assert!(index_source.contains("wasm_hash"));

    let mut reloaded = Simulation::new(&config_root).unwrap();
    let loaded = reloaded.open_behavior(&behavior_id).unwrap();
    assert!(!loaded.summary.builtin);
    assert_eq!(loaded.summary.build_status, "built");
    assert_eq!(loaded.source, edited_source);

    reloaded
        .place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let turret_id = reloaded.selected_id.clone().unwrap();
    let second_copy = reloaded.edit_builtin_copy(&turret_id).unwrap();
    assert_ne!(
        second_copy.summary.id, behavior_id,
        "loaded project behavior ids should reserve the next generated behavior id"
    );
}

#[test]
fn minimum_devices_place_and_drill_mines_ore_with_builtin_loop_source() {
    let mut sim = test_sim("sim");

    for x in 20..=30 {
        sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
            .unwrap();
    }
    for x in 21..30 {
        sim.place_block(BlockKind::Conveyor, Pos { x, y: 30 }, Direction::East)
            .unwrap();
    }
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();

    let drill_id = sim.selected_id.clone().unwrap();
    let drill = sim.blocks.get(&drill_id).unwrap();
    assert_eq!(drill.kind, BlockKind::Drill);
    assert_eq!(drill.behavior_ref.as_deref(), Some("builtin.drill.basic"));

    let source = sim.open_behavior("builtin.drill.basic").unwrap();
    assert!(source.source.contains("if output_blocked return"));
    assert!(source.source.contains("mine"));

    let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
    let starting_core_ore = sim.blocks[&core_id].inventory.count(&ItemKind::Ore);
    sim.step_ticks(500);
    let delivered = sim.blocks[&core_id].inventory.count(&ItemKind::Ore);
    assert!(
        delivered > starting_core_ore,
        "drill ore should ride conveyors into the 4x4 core"
    );
}

#[test]
fn edited_drill_behavior_mines_and_belts_ore_to_four_by_four_core() {
    let mut sim = test_sim("sim");

    for x in 20..=30 {
        sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
            .unwrap();
    }
    sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
        .unwrap();
    for x in 21..30 {
        sim.place_block(BlockKind::Conveyor, Pos { x, y: 30 }, Direction::East)
            .unwrap();
    }
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let drill_id = sim.selected_id.clone().unwrap();
    let copied = sim.edit_builtin_copy(&drill_id).unwrap();
    let behavior_id = copied.summary.id.clone();
    let player_source = "if output_blocked return\nlog player drill online\nmine";

    sim.save_behavior(&behavior_id, player_source.to_string())
        .unwrap();
    let build = sim.build_behavior(&behavior_id).unwrap();
    assert!(build.success);
    assert_eq!(
        sim.open_behavior(&behavior_id).unwrap().source,
        player_source
    );
    assert_eq!(
        sim.blocks[&drill_id].behavior_ref.as_deref(),
        Some(behavior_id.as_str()),
        "editing the built-in drill should reassign the placed drill to the project behavior"
    );

    let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
    assert_eq!(
        sim.block_id_at(Pos { x: 33, y: 33 }),
        Some(core_id.clone()),
        "the destination core should expose its full 4x4 footprint to belts"
    );
    let starting_core_ore = sim.blocks[&core_id].inventory.count(&ItemKind::Ore);

    sim.step_ticks(500);

    assert!(
        sim.logs
            .iter()
            .any(|entry| { entry.source == drill_id && entry.message == "player drill online" }),
        "the player-edited drill code should actually execute through the Wasm host log API"
    );
    assert!(
        sim.blocks[&core_id].inventory.count(&ItemKind::Ore) > starting_core_ore,
        "ore mined by the edited drill behavior should ride conveyors into the 4x4 core"
    );
}

#[test]
fn behavior_build_compiles_wat_and_save_invalidates_cache() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let block_id = sim.selected_id.clone().unwrap();
    let source = sim.edit_builtin_copy(&block_id).unwrap();
    let behavior_id = source.summary.id;

    sim.save_behavior(&behavior_id, xac_wasm::wat_const_action(30))
        .unwrap();
    let result = sim.build_behavior(&behavior_id).unwrap();
    assert!(result.success);
    assert!(result.wasm_hash.is_some());
    assert!(sim.compiled_behaviors.contains_key(&behavior_id));

    sim.save_behavior(&behavior_id, "(module".to_string())
        .unwrap();
    assert!(!sim.compiled_behaviors.contains_key(&behavior_id));
    assert!(sim.behaviors[&behavior_id].wasm_hash.is_none());

    let result = sim.build_behavior(&behavior_id).unwrap();
    assert!(!result.success);
    assert!(result.message.contains("parse behavior source as WAT"));
}

#[test]
fn behavior_build_compiles_tiny_source_and_hot_reloads_it() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let block_id = sim.selected_id.clone().unwrap();
    let source = sim.edit_builtin_copy(&block_id).unwrap();
    let behavior_id = source.summary.id;

    sim.save_behavior(
        &behavior_id,
        r#"
            fn tick() {
              log("tiny ok");
              mine();
            }
            "#
        .to_string(),
    )
    .unwrap();
    let result = sim.build_behavior(&behavior_id).unwrap();
    assert!(result.success);
    assert!(result.wasm_hash.is_some());
    assert!(sim.compiled_behaviors.contains_key(&behavior_id));

    sim.fuel_banks.insert(block_id.clone(), 100.0);
    sim.step_ticks(1);

    assert!(
        sim.logs.iter().any(|entry| {
            entry.level == LogLevel::Info && entry.source == block_id && entry.message == "tiny ok"
        }),
        "Tiny behavior should hot-reload into the same Wasm runtime"
    );
}

#[test]
fn xac_script_writes_network_store_and_recompute_preserves_it() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    let source = sim.edit_builtin_copy(&router_id).unwrap();
    sim.save_behavior(&source.summary.id, "net_set 7 42".to_string())
        .unwrap();
    sim.fuel_banks.insert(router_id.clone(), 100.0);

    sim.step_ticks(1);

    let network_id = sim.blocks[&router_id].network_id.unwrap();
    assert_eq!(
        sim.networks[&network_id].store.get("7"),
        Some(&serde_json::Value::from(42))
    );

    sim.recompute_networks();

    let network_id = sim.blocks[&router_id].network_id.unwrap();
    assert_eq!(
        sim.networks[&network_id].store.get("7"),
        Some(&serde_json::Value::from(42))
    );
}

#[test]
fn xac_script_deletes_network_store_key_in_order() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    let source = sim.edit_builtin_copy(&router_id).unwrap();
    sim.save_behavior(
        &source.summary.id,
        "net_set 7 42\nnet_delete 7\nnet_set 8 9".to_string(),
    )
    .unwrap();
    sim.fuel_banks.insert(router_id.clone(), 100.0);

    sim.step_ticks(1);

    let network_id = sim.blocks[&router_id].network_id.unwrap();
    let store = &sim.networks[&network_id].store;
    assert_eq!(store.get("7"), None);
    assert_eq!(store.get("8"), Some(&serde_json::Value::from(9)));
}

#[test]
fn xac_script_log_writes_game_log() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let drill_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &drill_id, "log drill ready");
    sim.fuel_banks.insert(drill_id.clone(), 100.0);

    sim.step_ticks(1);

    assert!(
        sim.logs.iter().any(|entry| {
            entry.level == LogLevel::Info
                && entry.source == drill_id
                && entry.message == "drill ready"
        }),
        "behavior log output should be copied into the game log"
    );
}

#[test]
fn router_output_available_script_waits_for_free_destination() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &router_id, "if output_available east push east");
    sim.place_block(BlockKind::Conveyor, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let conveyor_id = sim.selected_id.clone().unwrap();

    sim.blocks
        .get_mut(&router_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ore, 1);
    sim.blocks
        .get_mut(&conveyor_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ore, 1);

    sim.step_ticks(1);

    assert_eq!(
        sim.blocks[&router_id].inventory.count(&ItemKind::Ore),
        1,
        "router script should not push when output_available east is false"
    );
    assert_eq!(sim.blocks[&conveyor_id].inventory.count(&ItemKind::Ore), 1);

    sim.blocks
        .get_mut(&conveyor_id)
        .unwrap()
        .inventory
        .remove(&ItemKind::Ore, 1);
    sim.fuel_banks.insert(router_id.clone(), 100.0);
    sim.step_ticks(1);

    assert_eq!(
        sim.blocks[&router_id].inventory.count(&ItemKind::Ore),
        0,
        "router script should push as soon as the east destination has space"
    );
    assert_eq!(sim.blocks[&conveyor_id].inventory.count(&ItemKind::Ore), 1);
}

#[test]
fn drill_script_outputs_selected_ore_kind_to_conveyor() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let drill_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &drill_id, "if ore_kind == ore output ore");
    sim.blocks
        .get_mut(&drill_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ore, 1);
    sim.place_block(BlockKind::Conveyor, Pos { x: 21, y: 30 }, Direction::East)
        .unwrap();
    let conveyor_id = sim.selected_id.clone().unwrap();
    sim.fuel_banks.insert(drill_id.clone(), 100.0);

    sim.step_ticks(1);

    assert_eq!(sim.blocks[&drill_id].inventory.count(&ItemKind::Ore), 0);
    assert_eq!(
        sim.blocks[&conveyor_id].inventory.count(&ItemKind::Ore),
        1,
        "drill script should use ore_kind and output to move stored ore into the belt"
    );
}

#[test]
fn router_item_script_only_pushes_requested_item() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &router_id,
        "if output_available ammo east push ammo east",
    );
    sim.place_block(BlockKind::Conveyor, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let conveyor_id = sim.selected_id.clone().unwrap();

    sim.blocks
        .get_mut(&router_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ore, 1);
    sim.fuel_banks.insert(router_id.clone(), 100.0);
    sim.step_ticks(1);

    assert_eq!(
        sim.blocks[&router_id].inventory.count(&ItemKind::Ore),
        1,
        "ammo-specific router code should not move ore"
    );
    assert_eq!(sim.blocks[&conveyor_id].inventory.count(&ItemKind::Ore), 0);

    sim.blocks
        .get_mut(&router_id)
        .unwrap()
        .inventory
        .remove(&ItemKind::Ore, 1);
    sim.blocks
        .get_mut(&router_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 1);
    sim.fuel_banks.insert(router_id.clone(), 100.0);
    sim.step_ticks(1);

    assert_eq!(sim.blocks[&router_id].inventory.count(&ItemKind::Ammo), 0);
    assert_eq!(sim.blocks[&conveyor_id].inventory.count(&ItemKind::Ammo), 1);
}

#[test]
fn router_script_can_read_core_network_stock() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &router_id,
        "if stock_count ammo > 50 push ammo east",
    );
    sim.place_block(BlockKind::Conveyor, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let conveyor_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&router_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 1);
    sim.fuel_banks.insert(router_id.clone(), 100.0);

    sim.step_ticks(1);

    assert_eq!(sim.blocks[&router_id].inventory.count(&ItemKind::Ammo), 0);
    assert_eq!(
        sim.blocks[&conveyor_id].inventory.count(&ItemKind::Ammo),
        1,
        "router code should read same-network core stock and route ammo"
    );
}

#[test]
fn scripted_mining_factory_feeds_turret_and_defends_core() {
    let mut sim = test_sim("sim");

    for x in 20..=30 {
        sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
            .unwrap();
    }
    sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
        .unwrap();
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let drill_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &drill_id, "if output_blocked return\nmine");

    sim.place_block(BlockKind::Conveyor, Pos { x: 21, y: 30 }, Direction::East)
        .unwrap();
    sim.place_block(BlockKind::Router, Pos { x: 22, y: 30 }, Direction::East)
        .unwrap();
    let router_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &router_id, "if output_available east push east");

    sim.place_block(BlockKind::Assembler, Pos { x: 23, y: 30 }, Direction::East)
        .unwrap();
    let assembler_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &assembler_id,
        "set_recipe ammo\nif can_produce produce",
    );

    sim.place_block(BlockKind::Turret, Pos { x: 24, y: 30 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &turret_id, "if ammo_count > 0 attack_nearest");

    sim.step_ticks(800);

    assert!(
        sim.blocks[&turret_id].inventory.count(&ItemKind::Ammo) > 0,
        "ore should be mined, routed, assembled into ammo, and delivered into the turret"
    );

    let enemy_id = sim.make_id("enemy");
    sim.enemies.insert(
        enemy_id.clone(),
        combat::enemy_at(
            enemy_id.clone(),
            EnemyKind::Grunt,
            WorldPos { x: 25.5, y: 30.5 },
        ),
    );

    sim.step_ticks(80);

    assert!(
        !sim.enemies.contains_key(&enemy_id),
        "scripted turret should consume factory-made ammo and destroy the nearby enemy"
    );
}

#[test]
fn cpu_node_increases_wasm_driven_drill_throughput() {
    fn setup(with_cpu_node: bool) -> (Simulation, EntityId) {
        let mut sim = test_sim("sim");
        if with_cpu_node {
            for x in 20..=30 {
                sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
                    .unwrap();
            }
            sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
                .unwrap();
        }
        sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
            .unwrap();
        let drill_id = sim.selected_id.clone().unwrap();
        (sim, drill_id)
    }

    let (mut slow_sim, slow_drill_id) = setup(false);
    let (mut fast_sim, fast_drill_id) = setup(true);

    slow_sim.step_ticks(260);
    fast_sim.step_ticks(260);

    let slow_rate = slow_sim.blocks[&slow_drill_id].effective_cpu_rate;
    let fast_rate = fast_sim.blocks[&fast_drill_id].effective_cpu_rate;
    let slow_ore = slow_sim.blocks[&slow_drill_id]
        .inventory
        .count(&ItemKind::Ore);
    let fast_ore = fast_sim.blocks[&fast_drill_id]
        .inventory
        .count(&ItemKind::Ore);

    assert!(fast_rate > slow_rate);
    assert!(
        fast_ore > slow_ore,
        "cpu node should increase WAT-driven drill throughput: slow={slow_ore}, fast={fast_ore}"
    );
}

#[test]
fn local_cpu_banks_fuel_until_api_heavy_behavior_can_run() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 42, y: 32 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &turret_id,
        "if ammo_count > 0 attack_best lowest_hp",
    );
    sim.blocks
        .get_mut(&turret_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 3);
    let enemy_id = sim.make_id("enemy");
    let mut enemy = combat::enemy_at(
        enemy_id.clone(),
        EnemyKind::Grunt,
        WorldPos { x: 43.5, y: 32.5 },
    );
    enemy.move_speed = 0.0;
    sim.enemies.insert(enemy_id.clone(), enemy);
    let starting_hp = sim.enemies[&enemy_id].hp;

    sim.step_ticks(20);
    assert_eq!(
        sim.enemies[&enemy_id].hp, starting_hp,
        "local CPU should bank fuel instead of running API-heavy code immediately"
    );

    sim.step_ticks(300);
    assert!(
        sim.enemies
            .get(&enemy_id)
            .map(|enemy| enemy.hp < starting_hp)
            .unwrap_or(true),
        "after banking enough local CPU fuel, attack_best should run through Wasm host API"
    );
}

#[test]
fn assembler_builtin_calls_host_api_and_produces_ammo() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Assembler, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let assembler_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&assembler_id)
        .unwrap()
        .inventory
        .add(ItemKind::Plate, 1);

    sim.step_ticks(40);

    let assembler = &sim.blocks[&assembler_id];
    assert_eq!(assembler.recipe.as_deref(), Some("ammo"));
    assert!(
        assembler.inventory.count(&ItemKind::Ammo) > 0,
        "assembler builtin should call set_recipe/can_produce/produce through Wasm host imports"
    );
}

#[test]
fn assembler_recipe_goal_builds_missing_intermediate_from_assets() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Assembler, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let assembler_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&assembler_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ore, 2);

    sim.step_ticks(80);

    let assembler = &sim.blocks[&assembler_id];
    assert_eq!(assembler.recipe.as_deref(), Some("ammo"));
    assert!(
        assembler.inventory.count(&ItemKind::Ammo) > 0,
        "ammo goal should use assets/recipes.toml to make missing plate before ammo"
    );
}

#[test]
fn assembler_script_uses_output_count_to_switch_recipe_goal() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Assembler, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let assembler_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &assembler_id,
        "set_recipe plate\nif output_count ammo < 5 set_recipe ammo\nif can_produce produce",
    );
    {
        let assembler = sim.blocks.get_mut(&assembler_id).unwrap();
        assembler.inventory.add(ItemKind::Ammo, 5);
        assembler.inventory.add(ItemKind::Ore, 2);
    }

    sim.step_ticks(80);

    let assembler = &sim.blocks[&assembler_id];
    assert_eq!(assembler.recipe.as_deref(), Some("plate"));
    assert!(
        assembler.inventory.count(&ItemKind::Plate) > 0,
        "custom assembler code should read output_count and choose plate when ammo is stocked"
    );
}

#[test]
fn assembler_script_can_read_current_recipe_without_producing() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Assembler, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let assembler_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &assembler_id,
        "if current_recipe == ammo set_recipe plate",
    );
    sim.blocks.get_mut(&assembler_id).unwrap().recipe = Some("ammo".to_string());
    sim.fuel_banks.insert(assembler_id.clone(), 100.0);

    sim.step_ticks(1);

    let assembler = &sim.blocks[&assembler_id];
    assert_eq!(assembler.recipe.as_deref(), Some("plate"));
    assert_eq!(
        assembler.inventory.total(),
        0,
        "set_recipe should update the assembler goal without requiring produce"
    );
}

#[test]
fn turret_builtin_calls_host_api_and_attacks_enemy() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&turret_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 3);
    let enemy_id = sim.make_id("enemy");
    sim.enemies.insert(
        enemy_id.clone(),
        combat::enemy_at(
            enemy_id.clone(),
            EnemyKind::Grunt,
            WorldPos { x: 35.5, y: 32.5 },
        ),
    );

    sim.step_ticks(40);

    let enemy_hp = sim
        .enemies
        .get(&enemy_id)
        .map(|enemy| enemy.hp)
        .unwrap_or(0);
    assert!(
        enemy_hp < 30,
        "turret builtin should call attack_nearest through Wasm host imports"
    );
}

#[test]
fn turret_priority_script_targets_wire_cutter_before_nearest_grunt() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &turret_id,
        "if ammo_count > 0 attack_best wire_cutter runner armored nearest",
    );
    sim.blocks
        .get_mut(&turret_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 3);
    sim.fuel_banks.insert(turret_id.clone(), 100.0);

    let grunt_id = sim.make_id("enemy");
    let mut grunt = combat::enemy_at(
        grunt_id.clone(),
        EnemyKind::Grunt,
        WorldPos { x: 35.5, y: 32.5 },
    );
    grunt.move_speed = 0.0;
    sim.enemies.insert(grunt_id.clone(), grunt);
    let cutter_id = sim.make_id("enemy");
    let mut cutter = combat::enemy_at(
        cutter_id.clone(),
        EnemyKind::WireCutter,
        WorldPos { x: 38.5, y: 32.5 },
    );
    cutter.move_speed = 0.0;
    sim.enemies.insert(cutter_id.clone(), cutter);

    sim.step_ticks(1);

    assert_eq!(
        sim.enemies[&grunt_id].hp, 30,
        "nearest grunt should not be targeted while a prioritized wire_cutter is in range"
    );
    assert!(
        sim.enemies[&cutter_id].hp < 38,
        "custom turret code should prioritize wire_cutter before nearest"
    );
}

#[test]
fn turret_scan_script_attacks_requested_visible_enemy_index() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &turret_id, "if can_attack 1 attack 1");
    sim.blocks
        .get_mut(&turret_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 2);
    sim.fuel_banks.insert(turret_id.clone(), 100.0);

    let nearest_id = sim.make_id("enemy");
    let mut nearest = combat::enemy_at(
        nearest_id.clone(),
        EnemyKind::Grunt,
        WorldPos { x: 35.5, y: 32.5 },
    );
    nearest.move_speed = 0.0;
    sim.enemies.insert(nearest_id.clone(), nearest);
    let second_id = sim.make_id("enemy");
    let mut second = combat::enemy_at(
        second_id.clone(),
        EnemyKind::Runner,
        WorldPos { x: 36.5, y: 32.5 },
    );
    second.move_speed = 0.0;
    sim.enemies.insert(second_id.clone(), second);

    sim.step_ticks(1);

    assert_eq!(
        sim.enemies[&nearest_id].hp, 30,
        "scan index 0 should remain untouched"
    );
    assert_eq!(
        sim.enemies[&second_id].hp, 8,
        "scan index 1 should be attacked by custom turret code"
    );
}

#[test]
fn turret_scan_info_script_targets_runner_by_kind() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Turret, Pos { x: 34, y: 32 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &turret_id, "if enemy_kind 1 == runner attack 1");
    sim.blocks
        .get_mut(&turret_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 2);
    sim.fuel_banks.insert(turret_id.clone(), 100.0);

    let nearest_id = sim.make_id("enemy");
    let mut nearest = combat::enemy_at(
        nearest_id.clone(),
        EnemyKind::Grunt,
        WorldPos { x: 35.5, y: 32.5 },
    );
    nearest.move_speed = 0.0;
    sim.enemies.insert(nearest_id.clone(), nearest);
    let runner_id = sim.make_id("enemy");
    let mut runner = combat::enemy_at(
        runner_id.clone(),
        EnemyKind::Runner,
        WorldPos { x: 36.5, y: 32.5 },
    );
    runner.move_speed = 0.0;
    sim.enemies.insert(runner_id.clone(), runner);

    sim.step_ticks(1);

    assert_eq!(
        sim.enemies[&nearest_id].hp, 30,
        "scan index 0 should not be attacked when enemy_kind(1) matches runner"
    );
    assert_eq!(
        sim.enemies[&runner_id].hp, 8,
        "script should read scan metadata and attack the runner at index 1"
    );
}

#[test]
fn drone_port_builtin_delivers_core_ammo_to_turret_and_returns_home() {
    let mut sim = test_sim("sim");
    let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
    let starting_core_ammo = sim.blocks[&core_id].inventory.count(&ItemKind::Ammo);

    sim.place_block(BlockKind::DronePort, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let port_id = sim.selected_id.clone().unwrap();
    let port_source = sim.open_behavior("builtin.drone_port.basic").unwrap();
    assert!(port_source.source.contains("stock_count ammo"));
    assert!(port_source.source.contains("create_delivery_job ammo"));
    assert!(port_source.source.contains("dispatch_idle_drones"));
    sim.place_block(BlockKind::Turret, Pos { x: 42, y: 30 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();

    sim.step_ticks(360);

    assert_eq!(
        sim.drones.len(),
        1,
        "one drone_port should maintain one carrier drone instead of spawning every time it leaves"
    );
    let drone = sim.drones.values().next().unwrap();
    assert_eq!(drone.home_port, port_id);
    assert_eq!(
        drone.behavior_ref.as_deref(),
        Some("builtin.carrier_drone.basic")
    );
    assert_eq!(drone.state, DroneState::Docked);
    assert!(drone.job.is_none());
    assert!(
        sim.pending_jobs.is_empty(),
        "delivery job should be consumed after the turret receives ammo"
    );
    assert!(
        sim.blocks[&turret_id].inventory.count(&ItemKind::Ammo) >= 10,
        "carrier drone should deliver core ammo to the turret"
    );
    assert!(
        sim.blocks[&core_id].inventory.count(&ItemKind::Ammo) < starting_core_ammo,
        "delivery should remove ammo from core storage"
    );
}

#[test]
fn drone_port_script_uses_docked_and_pending_counts_to_dispatch() {
    let mut sim = test_sim("sim");

    sim.place_block(BlockKind::DronePort, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let port_id = sim.selected_id.clone().unwrap();
    assign_script(
        &mut sim,
        &port_id,
        "\
if docked_drone_count > 0 charge_docked_drones
if pending_job_count > 0 dispatch_idle_drones",
    );
    sim.place_block(BlockKind::Turret, Pos { x: 42, y: 30 }, Direction::East)
        .unwrap();

    sim.ensure_drone_and_job(&port_id);
    assert_eq!(sim.docked_drone_count_at_port(&port_id), 1);
    assert_eq!(sim.pending_jobs.len(), 1);
    let drone_id = sim.drones.values().next().unwrap().id.clone();
    sim.drones.get_mut(&drone_id).unwrap().battery = 50.0;
    sim.fuel_banks.insert(port_id.clone(), 100.0);

    sim.step_ticks(1);

    let drone = sim.drones.get(&drone_id).unwrap();
    assert!(
        drone.battery > 50.0,
        "docked_drone_count should let the script charge its docked carrier"
    );
    assert!(
        drone.job.is_some(),
        "pending_job_count should let the script dispatch the idle carrier"
    );
    assert!(
        sim.pending_jobs.is_empty(),
        "dispatch should consume the queued delivery job"
    );
}

#[test]
fn docked_carrier_drone_banks_wasm_fuel_from_home_network_cpu() {
    fn setup(port_pos: Pos, turret_pos: Pos) -> (Simulation, EntityId) {
        let mut sim = test_sim("sim");
        let core_id = sim.block_id_at(Pos { x: 30, y: 30 }).unwrap();
        sim.place_block(BlockKind::DronePort, port_pos, Direction::East)
            .unwrap();
        let port_id = sim.selected_id.clone().unwrap();
        sim.blocks.get_mut(&port_id).unwrap().behavior_ref = None;
        sim.place_block(BlockKind::Turret, turret_pos, Direction::East)
            .unwrap();
        let turret_id = sim.selected_id.clone().unwrap();
        let behavior_id = install_test_drone_behavior(&mut sim);
        let drone_id = sim.make_id("drone");
        let port = sim.blocks[&port_id].clone();
        sim.drones.insert(
            drone_id.clone(),
            Drone {
                id: drone_id.clone(),
                home_port: port_id,
                behavior_ref: Some(behavior_id),
                pos: geometry::block_center(&port),
                battery: 100.0,
                logic_fuel: 1000,
                cargo: Inventory::with_capacity(20),
                state: DroneState::Docked,
                job: None,
            },
        );
        let job_id = sim.make_id("job");
        sim.pending_jobs.push(DeliveryJob {
            id: job_id,
            item: ItemKind::Ammo,
            amount: 10,
            pickup: core_id,
            dropoff: turret_id,
            priority: 50,
        });
        sim.recompute_networks();
        (sim, drone_id)
    }

    let (mut connected, connected_drone_id) = setup(Pos { x: 34, y: 30 }, Pos { x: 42, y: 30 });
    let (mut isolated, isolated_drone_id) = setup(Pos { x: 42, y: 42 }, Pos { x: 44, y: 42 });

    connected.step_ticks(25);
    isolated.step_ticks(25);

    assert!(
            connected.drones[&connected_drone_id].job.is_some(),
            "docked drone on the core network should bank enough network CPU fuel to run its Wasm behavior"
        );
    assert!(
        isolated.drones[&isolated_drone_id].job.is_none(),
        "docked drone on a small isolated drone_port network should still be banking fuel"
    );
}

#[test]
fn destroyed_drone_port_cleans_home_drone_and_delivery_jobs() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::DronePort, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let port_id = sim.selected_id.clone().unwrap();
    sim.blocks.get_mut(&port_id).unwrap().behavior_ref = None;
    sim.place_block(BlockKind::Turret, Pos { x: 42, y: 30 }, Direction::East)
        .unwrap();
    let turret_id = sim.selected_id.clone().unwrap();
    let drone_id = sim.make_id("drone");
    let port = sim.blocks[&port_id].clone();
    sim.drones.insert(
        drone_id.clone(),
        Drone {
            id: drone_id.clone(),
            home_port: port_id.clone(),
            behavior_ref: Some("builtin.carrier_drone.basic".to_string()),
            pos: geometry::block_center(&port),
            battery: 100.0,
            logic_fuel: 1000,
            cargo: Inventory::with_capacity(20),
            state: DroneState::Docked,
            job: None,
        },
    );
    sim.fuel_banks.insert(drone_id.clone(), 50.0);
    let job_id = sim.make_id("job");
    sim.pending_jobs.push(DeliveryJob {
        id: job_id,
        item: ItemKind::Ammo,
        amount: 10,
        pickup: port_id.clone(),
        dropoff: turret_id,
        priority: 50,
    });
    sim.blocks.get_mut(&port_id).unwrap().hp = 0;

    sim.step_ticks(1);

    assert!(!sim.blocks.contains_key(&port_id));
    assert!(
        !sim.drones.contains_key(&drone_id),
        "destroying a drone_port should remove its home carrier"
    );
    assert!(!sim.fuel_banks.contains_key(&drone_id));
    assert!(
        sim.pending_jobs.is_empty(),
        "delivery jobs referencing the destroyed port should be removed"
    );
}

#[test]
fn carrier_drone_low_level_script_loads_moves_and_unloads() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Storage, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let pickup_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&pickup_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 6);
    sim.place_block(BlockKind::Storage, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let dropoff_id = sim.selected_id.clone().unwrap();
    let behavior_id = install_test_drone_behavior_source(&mut sim, "load ammo 5");
    let drone_id = sim.make_id("drone");
    let pickup = sim.blocks[&pickup_id].clone();
    sim.drones.insert(
        drone_id.clone(),
        Drone {
            id: drone_id.clone(),
            home_port: pickup_id.clone(),
            behavior_ref: Some(behavior_id.clone()),
            pos: geometry::block_center(&pickup),
            battery: 100.0,
            logic_fuel: 1000,
            cargo: Inventory::with_capacity(20),
            state: DroneState::Docked,
            job: None,
        },
    );
    sim.fuel_banks.insert(drone_id.clone(), 100.0);

    sim.step_ticks(1);

    assert_eq!(sim.drones[&drone_id].cargo.count(&ItemKind::Ammo), 5);
    assert_eq!(sim.blocks[&pickup_id].inventory.count(&ItemKind::Ammo), 1);

    set_test_drone_behavior_source(
        &mut sim,
        &behavior_id,
        "if cargo_count ammo > 0 move_to 35 30",
    );
    sim.fuel_banks.insert(drone_id.clone(), 100.0);
    sim.step_ticks(400);
    let dropoff_center = geometry::block_center(&sim.blocks[&dropoff_id]);
    assert!(
        sim.drones[&drone_id].pos.distance(dropoff_center) <= 0.2,
        "move_to should drive the free-moving drone toward the requested tile"
    );

    set_test_drone_behavior_source(
        &mut sim,
        &behavior_id,
        "if cargo_count ammo > 0 unload ammo 5",
    );
    sim.fuel_banks.insert(drone_id.clone(), 100.0);
    sim.step_ticks(1);

    assert_eq!(sim.drones[&drone_id].cargo.count(&ItemKind::Ammo), 0);
    assert_eq!(sim.blocks[&dropoff_id].inventory.count(&ItemKind::Ammo), 5);
}

#[test]
fn carrier_drone_low_level_physical_commands_require_battery() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Storage, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let pickup_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&pickup_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 6);
    sim.place_block(BlockKind::Storage, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let dropoff_id = sim.selected_id.clone().unwrap();
    let behavior_id = install_test_drone_behavior_source(&mut sim, "load ammo 5");
    let drone_id = sim.make_id("drone");
    let pickup_center = geometry::block_center(&sim.blocks[&pickup_id]);
    sim.drones.insert(
        drone_id.clone(),
        Drone {
            id: drone_id.clone(),
            home_port: pickup_id.clone(),
            behavior_ref: Some(behavior_id.clone()),
            pos: pickup_center,
            battery: 0.0,
            logic_fuel: 1000,
            cargo: Inventory::with_capacity(20),
            state: DroneState::Docked,
            job: None,
        },
    );
    sim.fuel_banks.insert(drone_id.clone(), 100.0);

    sim.step_ticks(1);

    assert_eq!(
        sim.drones[&drone_id].cargo.count(&ItemKind::Ammo),
        0,
        "load should not move inventory into cargo with an empty battery"
    );
    assert_eq!(
        sim.blocks[&pickup_id].inventory.count(&ItemKind::Ammo),
        6,
        "load should not remove block inventory with an empty battery"
    );

    set_test_drone_behavior_source(&mut sim, &behavior_id, "move_to 35 30");
    let start_pos = sim.drones[&drone_id].pos;
    sim.drones.get_mut(&drone_id).unwrap().battery = 0.0;
    sim.fuel_banks.insert(drone_id.clone(), 100.0);

    sim.step_ticks(1);

    assert!(
        sim.drones[&drone_id].pos.distance(start_pos) <= f32::EPSILON,
        "move_to should not change position with an empty battery"
    );

    set_test_drone_behavior_source(&mut sim, &behavior_id, "unload ammo 5");
    {
        let drone = sim.drones.get_mut(&drone_id).unwrap();
        drone.pos = geometry::block_center(&sim.blocks[&dropoff_id]);
        drone.battery = 0.0;
        drone.cargo.add(ItemKind::Ammo, 5);
    }
    sim.fuel_banks.insert(drone_id.clone(), 100.0);

    sim.step_ticks(1);

    assert_eq!(
        sim.drones[&drone_id].cargo.count(&ItemKind::Ammo),
        5,
        "unload should leave cargo untouched with an empty battery"
    );
    assert_eq!(
        sim.blocks[&dropoff_id].inventory.count(&ItemKind::Ammo),
        0,
        "unload should not mutate block inventory with an empty battery"
    );
}

#[test]
fn carrier_drone_delivery_job_requires_battery_before_moving() {
    let mut sim = test_sim("sim");
    sim.place_block(BlockKind::Storage, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let pickup_id = sim.selected_id.clone().unwrap();
    sim.blocks
        .get_mut(&pickup_id)
        .unwrap()
        .inventory
        .add(ItemKind::Ammo, 6);
    sim.place_block(BlockKind::Storage, Pos { x: 35, y: 30 }, Direction::East)
        .unwrap();
    let dropoff_id = sim.selected_id.clone().unwrap();
    let drone_id = sim.make_id("drone");
    let start_pos = geometry::block_center(&sim.blocks[&dropoff_id]);
    let job_id = sim.make_id("job");
    sim.drones.insert(
        drone_id.clone(),
        Drone {
            id: drone_id.clone(),
            home_port: pickup_id.clone(),
            behavior_ref: None,
            pos: start_pos,
            battery: 0.0,
            logic_fuel: 1000,
            cargo: Inventory::with_capacity(20),
            state: DroneState::Delivering,
            job: Some(DeliveryJob {
                id: job_id,
                item: ItemKind::Ammo,
                amount: 5,
                pickup: pickup_id.clone(),
                dropoff: dropoff_id,
                priority: 50,
            }),
        },
    );

    sim.step_ticks(1);

    let drone = &sim.drones[&drone_id];
    assert!(
        drone.pos.distance(start_pos) <= f32::EPSILON,
        "delivery job movement should not advance with an empty battery"
    );
    assert_eq!(drone.state, DroneState::Offline);
    assert_eq!(drone.cargo.count(&ItemKind::Ammo), 0);
    assert_eq!(sim.blocks[&pickup_id].inventory.count(&ItemKind::Ammo), 6);
}

#[test]
fn wire_cutter_breaks_wire_and_splits_cpu_network() {
    let mut sim = test_sim("sim");
    for x in 20..=30 {
        sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
            .unwrap();
    }
    sim.place_block(BlockKind::CpuNode, Pos { x: 19, y: 29 }, Direction::East)
        .unwrap();
    sim.place_block(BlockKind::Drill, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let drill_id = sim.selected_id.clone().unwrap();
    let wire_id = sim.block_id_at(Pos { x: 20, y: 29 }).unwrap();

    sim.step_ticks(1);
    let connected_rate = sim.blocks[&drill_id].effective_cpu_rate;
    assert!(connected_rate > 100.0);

    let enemy_id = sim.make_id("enemy");
    sim.enemies.insert(
        enemy_id.clone(),
        combat::enemy_at(
            enemy_id,
            EnemyKind::WireCutter,
            WorldPos { x: 20.5, y: 29.5 },
        ),
    );
    let status = sim.snapshot().status;
    assert_eq!(status.wire_threats, 1);
    assert_eq!(status.network_cpu, 200.0);

    sim.step_ticks(2);
    assert_eq!(
        sim.blocks[&wire_id].hp,
        BlockKind::Wire.max_hp() - EnemyKind::WireCutter.attack_damage(),
        "wire cutter should not apply damage every tick while attack cooldown is active"
    );

    sim.step_ticks(40);

    assert!(
        !sim.blocks.contains_key(&wire_id),
        "wire cutter should destroy the targeted wire"
    );
    let disconnected_rate = sim.blocks[&drill_id].effective_cpu_rate;
    assert!(
            disconnected_rate < connected_rate,
            "destroying a wire should lower drill CPU by splitting the network: before={connected_rate}, after={disconnected_rate}"
        );
    assert_eq!(
        sim.blocks[&drill_id].network_id, None,
        "drill should fall back to local CPU when wire is cut"
    );
}

#[test]
fn reconnecting_split_network_keeps_core_store_over_read_only_cache() {
    let mut sim = test_sim("sim");
    for x in 20..=30 {
        sim.place_block(BlockKind::Wire, Pos { x, y: 29 }, Direction::East)
            .unwrap();
    }
    sim.place_block(BlockKind::Router, Pos { x: 20, y: 30 }, Direction::East)
        .unwrap();
    let remote_router_id = sim.selected_id.clone().unwrap();
    sim.place_block(BlockKind::Router, Pos { x: 34, y: 30 }, Direction::East)
        .unwrap();
    let core_router_id = sim.selected_id.clone().unwrap();
    assign_script(&mut sim, &core_router_id, "net_set 7 10");
    sim.fuel_banks.insert(core_router_id.clone(), 100.0);

    sim.step_ticks(1);
    let network_id = sim.blocks[&core_router_id].network_id.unwrap();
    assert_eq!(
        sim.networks[&network_id].store.get("7"),
        Some(&serde_json::Value::from(10))
    );

    let split_wire_id = sim.block_id_at(Pos { x: 29, y: 29 }).unwrap();
    sim.deconstruct_block(&split_wire_id).unwrap();
    assign_script(&mut sim, &remote_router_id, "net_set 7 42");
    set_block_behavior_source(&mut sim, &core_router_id, "net_set 7 99");
    sim.fuel_banks.insert(remote_router_id.clone(), 100.0);
    sim.fuel_banks.insert(core_router_id.clone(), 100.0);

    sim.step_ticks(1);

    let core_network_id = sim.blocks[&core_router_id].network_id.unwrap();
    let remote_network_id = sim.blocks[&remote_router_id].network_id.unwrap();
    assert_ne!(core_network_id, remote_network_id);
    assert_eq!(
        sim.networks[&core_network_id].store.get("7"),
        Some(&serde_json::Value::from(99))
    );
    assert_eq!(
        sim.networks[&remote_network_id].store.get("7"),
        Some(&serde_json::Value::from(10)),
        "the split side should keep only a read-only snapshot"
    );
    assert!(sim.networks[&remote_network_id].read_only_cache);

    sim.place_block(BlockKind::Wire, Pos { x: 29, y: 29 }, Direction::East)
        .unwrap();

    let reconnected_network_id = sim.blocks[&core_router_id].network_id.unwrap();
    assert_eq!(
        sim.blocks[&remote_router_id].network_id,
        Some(reconnected_network_id)
    );
    assert_eq!(
        sim.networks[&reconnected_network_id].store.get("7"),
        Some(&serde_json::Value::from(99)),
        "reconnecting should keep the core-backed store over the larger stale cache"
    );
}

fn assign_script(sim: &mut Simulation, block_id: &str, source: &str) {
    let behavior = sim.edit_builtin_copy(block_id).unwrap();
    let result = sim
        .build_behavior(&behavior.summary.id)
        .expect("copied builtin should build");
    assert!(result.success);
    sim.save_behavior(&behavior.summary.id, source.to_string())
        .unwrap();
    let result = sim
        .build_behavior(&behavior.summary.id)
        .expect("custom XaC script should build");
    assert!(result.success);
}

fn set_block_behavior_source(sim: &mut Simulation, block_id: &str, source: &str) {
    let behavior_id = sim.blocks[block_id].behavior_ref.clone().unwrap();
    sim.save_behavior(&behavior_id, source.to_string()).unwrap();
    let result = sim.build_behavior(&behavior_id).unwrap();
    assert!(result.success);
}

fn install_test_drone_behavior(sim: &mut Simulation) -> BehaviorId {
    install_test_drone_behavior_source(sim, &xac_wasm::wat_const_action(51))
}

fn install_test_drone_behavior_source(sim: &mut Simulation, source: &str) -> BehaviorId {
    let behavior_id = sim.make_id("behavior");
    sim.behaviors.insert(
        behavior_id.clone(),
        BehaviorPackage {
            summary: BehaviorSummary {
                id: behavior_id.clone(),
                display_name: "Test Drone Claim".to_string(),
                base_kind: BehaviorKind::CarrierDrone,
                world: "carrier-drone-behavior".to_string(),
                builtin: false,
                used_by: 0,
                source_path: "test://carrier-drone-claim.wat".to_string(),
                build_status: "test".to_string(),
            },
            source: source.to_string(),
            wasm_hash: None,
        },
    );
    behavior_id
}

fn set_test_drone_behavior_source(sim: &mut Simulation, behavior_id: &str, source: &str) {
    let package = sim.behaviors.get_mut(behavior_id).unwrap();
    package.source = source.to_string();
    package.wasm_hash = None;
    sim.compiled_behaviors.remove(behavior_id);
}

fn test_sim(name: &str) -> Simulation {
    Simulation::new(test_config_root(name)).unwrap()
}

fn test_config_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let serial = TEST_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("xac-{name}-{nanos}-{serial}"));
    fs::create_dir_all(&path).unwrap();
    path
}
