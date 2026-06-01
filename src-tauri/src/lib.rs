use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use xac_core::{BehaviorSource, BlockKind, BuildResult, Direction, GameSnapshot, Pos};
use xac_sim::Simulation;

struct AppState {
    sim: Mutex<Simulation>,
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> Result<GameSnapshot, String> {
    let sim = state.sim.lock().map_err(|err| err.to_string())?;
    Ok(sim.snapshot())
}

#[tauri::command]
fn set_running(state: tauri::State<'_, AppState>, running: bool) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    Ok(sim.set_running(running))
}

#[tauri::command]
fn step_ticks(state: tauri::State<'_, AppState>, count: u32) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    Ok(sim.step_ticks(count))
}

#[tauri::command]
fn advance(state: tauri::State<'_, AppState>, max_ticks: u32) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    Ok(sim.update_if_running(max_ticks))
}

#[tauri::command]
fn place_block(
    state: tauri::State<'_, AppState>,
    kind: BlockKind,
    x: i32,
    y: i32,
    dir: Direction,
) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.place_block(kind, Pos { x, y }, dir)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn deconstruct_block(
    state: tauri::State<'_, AppState>,
    block_id: String,
) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.deconstruct_block(&block_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn rotate_block(
    state: tauri::State<'_, AppState>,
    block_id: String,
) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.rotate_block(&block_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn select_entity(
    state: tauri::State<'_, AppState>,
    id: Option<String>,
) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    Ok(sim.select_entity(id))
}

#[tauri::command]
fn open_behavior(
    state: tauri::State<'_, AppState>,
    behavior_id: String,
) -> Result<BehaviorSource, String> {
    let sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.open_behavior(&behavior_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn edit_builtin_copy(
    state: tauri::State<'_, AppState>,
    block_id: String,
) -> Result<BehaviorSource, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.edit_builtin_copy(&block_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn fork_behavior(
    state: tauri::State<'_, AppState>,
    block_id: String,
) -> Result<BehaviorSource, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.fork_behavior(&block_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn assign_behavior(
    state: tauri::State<'_, AppState>,
    block_id: String,
    behavior_id: String,
) -> Result<GameSnapshot, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.assign_behavior(&block_id, &behavior_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn save_behavior(
    state: tauri::State<'_, AppState>,
    behavior_id: String,
    source: String,
) -> Result<BehaviorSource, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.save_behavior(&behavior_id, source)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn build_behavior(
    state: tauri::State<'_, AppState>,
    behavior_id: String,
) -> Result<BuildResult, String> {
    let mut sim = state.sim.lock().map_err(|err| err.to_string())?;
    sim.build_behavior(&behavior_id)
        .map_err(|err| err.to_string())
}

pub fn run() {
    let config_root = config_root();
    fs::create_dir_all(config_root.join("projects/default_project/saves")).ok();
    fs::create_dir_all(config_root.join("cache/wasm")).ok();
    fs::create_dir_all(config_root.join("common/templates")).ok();

    let simulation = Simulation::new(&config_root).expect("initialize XaC simulation");

    tauri::Builder::default()
        .manage(AppState {
            sim: Mutex::new(simulation),
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_title("XaC - RTS as Code").ok();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            set_running,
            step_ticks,
            advance,
            place_block,
            deconstruct_block,
            rotate_block,
            select_entity,
            open_behavior,
            edit_builtin_copy,
            fork_behavior,
            assign_behavior,
            save_behavior,
            build_behavior
        ])
        .run(tauri::generate_context!())
        .expect("error while running XaC");
}

fn config_root() -> PathBuf {
    if let Ok(value) = env::var("XDG_CONFIG_HOME") {
        if !value.trim().is_empty() {
            return PathBuf::from(value).join("xac");
        }
    }
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".config/xac")
}
