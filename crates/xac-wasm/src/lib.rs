use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmtime::{Config, Instance, Module, Store};
use xac_core::{BlockKind, Direction, EnemyKind};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BehaviorIntent {
    Noop,
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
pub struct CompiledBehavior {
    kind: BlockKind,
    module: Module,
    wasm_hash: String,
}

impl CompiledBehavior {
    pub fn kind(&self) -> BlockKind {
        self.kind
    }

    pub fn wasm_hash(&self) -> &str {
        &self.wasm_hash
    }
}

#[derive(Clone)]
pub struct BehaviorRuntime {
    engine: wasmtime::Engine,
}

impl BehaviorRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.wasm_component_model(true);
        Ok(Self {
            engine: wasmtime::Engine::new(&config)?,
        })
    }

    pub fn compile_wat(&self, kind: BlockKind, source: &str) -> Result<CompiledBehavior> {
        let wasm = wat::parse_str(source).context("parse behavior WAT")?;
        let wasm_hash = hash_bytes(&wasm);
        let module = Module::new(&self.engine, &wasm).context("compile behavior wasm")?;

        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .context("instantiate behavior wasm for ABI validation")?;
        instance
            .get_typed_func::<(), i32>(&mut store, "tick")
            .context("behavior must export tick() -> i32")?;

        Ok(CompiledBehavior {
            kind,
            module,
            wasm_hash,
        })
    }

    pub fn evaluate_compiled(
        &self,
        compiled: &CompiledBehavior,
        fuel: u64,
    ) -> Result<BehaviorEval> {
        let mut store = Store::new(&self.engine, ());
        store.set_fuel(fuel).context("set behavior fuel")?;
        let instance = Instance::new(&mut store, &compiled.module, &[])
            .context("instantiate behavior wasm")?;
        let tick = instance
            .get_typed_func::<(), i32>(&mut store, "tick")
            .context("load tick export")?;

        let action_code = match tick.call(&mut store, ()) {
            Ok(code) => code,
            Err(_) => {
                let fuel_remaining = store.get_fuel().unwrap_or(0);
                return Ok(BehaviorEval {
                    intent: BehaviorIntent::Noop,
                    fuel_spent: fuel.saturating_sub(fuel_remaining),
                    fuel_remaining,
                    over_budget: true,
                    wasm_hash: compiled.wasm_hash.clone(),
                });
            }
        };
        let fuel_remaining = store.get_fuel().unwrap_or(0);
        let intent = action_code_to_intent(compiled.kind, action_code)?;

        Ok(BehaviorEval {
            intent,
            fuel_spent: fuel.saturating_sub(fuel_remaining),
            fuel_remaining,
            over_budget: false,
            wasm_hash: compiled.wasm_hash.clone(),
        })
    }
}

pub fn hash_wasm_source(source: &str) -> Result<String> {
    let wasm = wat::parse_str(source).context("parse behavior WAT")?;
    Ok(hash_bytes(&wasm))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn action_code_to_intent(kind: BlockKind, action_code: i32) -> Result<BehaviorIntent> {
    match action_code {
        0 => Ok(BehaviorIntent::Noop),
        1 if kind == BlockKind::Drill => Ok(BehaviorIntent::DrillDefault),
        10 if kind == BlockKind::Router => Ok(BehaviorIntent::Router {
            preferred: Direction::all().to_vec(),
        }),
        11 if kind == BlockKind::Router => router_dir(Direction::North),
        12 if kind == BlockKind::Router => router_dir(Direction::East),
        13 if kind == BlockKind::Router => router_dir(Direction::South),
        14 if kind == BlockKind::Router => router_dir(Direction::West),
        20 if kind == BlockKind::Assembler => Ok(BehaviorIntent::Assembler { prefer_ammo: false }),
        21 if kind == BlockKind::Assembler => Ok(BehaviorIntent::Assembler { prefer_ammo: true }),
        30 if kind == BlockKind::Turret => Ok(BehaviorIntent::Turret {
            priority: vec![TargetRule::Nearest],
        }),
        31 if kind == BlockKind::Turret => Ok(BehaviorIntent::Turret {
            priority: vec![TargetRule::LowestHp, TargetRule::Nearest],
        }),
        40 if kind == BlockKind::DronePort => Ok(BehaviorIntent::DronePort),
        code => Err(anyhow!(
            "behavior returned invalid action code {code} for {kind:?}"
        )),
    }
}

fn router_dir(dir: Direction) -> Result<BehaviorIntent> {
    Ok(BehaviorIntent::Router {
        preferred: vec![dir],
    })
}

pub fn wat_const_action(action_code: i32) -> String {
    format!(
        r#"(module
  (func (export "tick") (result i32)
    (i32.const {action_code})))"#
    )
}

pub fn wat_spin_action(spin_count: u32, action_code: i32) -> String {
    format!(
        r#"(module
  (func $spin (param $n i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $exit
      (loop $loop
        (br_if $exit (i32.ge_s (local.get $i) (local.get $n)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))
  (func (export "tick") (result i32)
    (call $spin (i32.const {spin_count}))
    (i32.const {action_code})))"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_wat_and_evaluates_action_code() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(BlockKind::Drill, &wat_const_action(1))
            .unwrap();
        let eval = runtime.evaluate_compiled(&compiled, 20).unwrap();

        assert!(matches!(eval.intent, BehaviorIntent::DrillDefault));
        assert!(!eval.over_budget);
        assert!(eval.fuel_spent > 0);
        assert_eq!(compiled.wasm_hash(), eval.wasm_hash);
    }

    #[test]
    fn rejects_invalid_wat_and_missing_tick() {
        let runtime = BehaviorRuntime::new().unwrap();

        assert!(runtime.compile_wat(BlockKind::Drill, "not wat").is_err());
        assert!(runtime.compile_wat(BlockKind::Drill, "(module)").is_err());
        assert!(runtime
            .compile_wat(
                BlockKind::Drill,
                r#"(module (func (export "tick") (result i64) (i64.const 1)))"#
            )
            .is_err());
    }

    #[test]
    fn reports_over_budget_when_fuel_is_exhausted() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(BlockKind::Drill, &wat_spin_action(10_000, 1))
            .unwrap();
        let eval = runtime.evaluate_compiled(&compiled, 1).unwrap();

        assert!(eval.over_budget);
        assert!(matches!(eval.intent, BehaviorIntent::Noop));
    }

    #[test]
    fn validates_action_code_against_block_kind() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(BlockKind::Drill, &wat_const_action(30))
            .unwrap();

        let err = runtime.evaluate_compiled(&compiled, 20).unwrap_err();
        assert!(err.to_string().contains("invalid action code 30"));
    }

    #[test]
    fn maps_router_and_assembler_actions() {
        assert!(matches!(
            action_code_to_intent(BlockKind::Router, 12).unwrap(),
            BehaviorIntent::Router { preferred } if preferred == vec![Direction::East]
        ));
        assert!(matches!(
            action_code_to_intent(BlockKind::Assembler, 21).unwrap(),
            BehaviorIntent::Assembler { prefer_ammo: true }
        ));
    }
}
