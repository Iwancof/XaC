use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmtime::{Config, Engine, Instance, Module, Store};
use xac_core::{BlockKind, Direction, EnemyKind, ItemKind};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BehaviorIntent {
    DrillDefault,
    Router { preferred: Vec<Direction> },
    Assembler { prefer_ammo: bool },
    Turret { priority: Vec<TargetRule> },
    DronePort,
    CarrierDrone,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TargetRule {
    Kind(EnemyKind),
    Nearest,
    LowestHp,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorEval {
    pub intent: BehaviorIntent,
    pub fuel_spent: u64,
    pub fuel_remaining: u64,
    pub over_budget: bool,
    pub wasm_hash: String,
}

#[derive(Clone)]
pub struct BehaviorRuntime {
    engine: Engine,
}

impl BehaviorRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.wasm_component_model(true);
        Ok(Self {
            engine: Engine::new(&config)?,
        })
    }

    pub fn evaluate(&self, kind: BlockKind, source: &str, fuel: u64) -> Result<BehaviorEval> {
        let intent = infer_intent(kind, source);
        let action_code = action_code(&intent);
        let wasm = wasm_for_action(action_code)?;
        let wasm_hash = hash_bytes(&wasm);
        let module = Module::new(&self.engine, &wasm).context("compile behavior wasm")?;
        let mut store = Store::new(&self.engine, ());
        store.set_fuel(fuel).context("set behavior fuel")?;
        let instance =
            Instance::new(&mut store, &module, &[]).context("instantiate behavior wasm")?;
        let tick = instance
            .get_typed_func::<(), i32>(&mut store, "tick")
            .context("load tick export")?;

        let call = tick.call(&mut store, ());
        let fuel_remaining = store.get_fuel().unwrap_or(0);
        let over_budget = call.is_err();

        Ok(BehaviorEval {
            intent,
            fuel_spent: fuel.saturating_sub(fuel_remaining),
            fuel_remaining,
            over_budget,
            wasm_hash,
        })
    }
}

pub fn hash_source(source: &str) -> String {
    hash_bytes(source.as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn wasm_for_action(action_code: i32) -> Result<Vec<u8>> {
    // This tiny core Wasm module is the MVP execution shim. It lets the
    // simulation spend real Wasmtime fuel while the authoring UI remains
    // source-oriented; the WIT/component ABI can replace the shim per world.
    let wat = format!(
        r#"(module
          (func $spin (param $n i32) (result i32)
            (local $i i32)
            (local.set $i (i32.const 0))
            (block $exit
              (loop $loop
                (br_if $exit (i32.ge_s (local.get $i) (local.get $n)))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $loop)))
            (local.get $i))
          (func (export "tick") (result i32)
            (drop (call $spin (i32.const 8)))
            (i32.const {action_code})))"#
    );
    wat::parse_str(wat).context("parse behavior wat")
}

fn action_code(intent: &BehaviorIntent) -> i32 {
    match intent {
        BehaviorIntent::DrillDefault => 1,
        BehaviorIntent::Router { .. } => 2,
        BehaviorIntent::Assembler { prefer_ammo } => {
            if *prefer_ammo {
                3
            } else {
                4
            }
        }
        BehaviorIntent::Turret { .. } => 5,
        BehaviorIntent::DronePort => 6,
        BehaviorIntent::CarrierDrone => 7,
    }
}

fn infer_intent(kind: BlockKind, source: &str) -> BehaviorIntent {
    let lower = source.to_ascii_lowercase();
    match kind {
        BlockKind::Router => BehaviorIntent::Router {
            preferred: infer_router_dirs(&lower),
        },
        BlockKind::Assembler => BehaviorIntent::Assembler {
            prefer_ammo: lower.contains("ammo"),
        },
        BlockKind::Turret => BehaviorIntent::Turret {
            priority: infer_target_priority(&lower),
        },
        BlockKind::DronePort => BehaviorIntent::DronePort,
        BlockKind::Drill => BehaviorIntent::DrillDefault,
        _ => BehaviorIntent::DrillDefault,
    }
}

fn infer_router_dirs(source: &str) -> Vec<Direction> {
    let mut dirs = Vec::new();
    for (needle, dir) in [
        ("north", Direction::North),
        ("east", Direction::East),
        ("south", Direction::South),
        ("west", Direction::West),
    ] {
        if source.contains(needle) {
            dirs.push(dir);
        }
    }
    if source.contains(ItemKind::Ammo.as_str()) && !dirs.contains(&Direction::East) {
        dirs.insert(0, Direction::East);
    }
    if dirs.is_empty() {
        dirs.extend(Direction::all());
    }
    dirs
}

fn infer_target_priority(source: &str) -> Vec<TargetRule> {
    let mut rules = Vec::new();
    let candidates = [
        ("runner", EnemyKind::Runner),
        ("wire_cutter", EnemyKind::WireCutter),
        ("wire-cutter", EnemyKind::WireCutter),
        ("armored", EnemyKind::Armored),
        ("grunt", EnemyKind::Grunt),
    ];
    for (needle, kind) in candidates {
        if source.contains(needle) {
            rules.push(TargetRule::Kind(kind));
        }
    }
    if source.contains("lowest_hp") || source.contains("lowest-hp") {
        rules.push(TargetRule::LowestHp);
    }
    rules.push(TargetRule::Nearest);
    rules
}
