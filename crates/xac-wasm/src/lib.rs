use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmtime::{Caller, Config, Instance, Linker, Module, Store};
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

#[derive(Clone, Debug, Default)]
pub struct BehaviorHostInput {
    pub output_blocked: bool,
    pub can_produce: bool,
    pub ammo_count: i32,
}

#[derive(Clone, Debug)]
struct BehaviorHostState {
    input: BehaviorHostInput,
    intent: BehaviorIntent,
    assembler_prefer_ammo: bool,
}

impl BehaviorHostState {
    fn new(input: BehaviorHostInput) -> Self {
        Self {
            input,
            intent: BehaviorIntent::Noop,
            assembler_prefer_ammo: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TickAbi {
    Void,
    ActionCode,
}

#[derive(Clone)]
pub struct CompiledBehavior {
    kind: BlockKind,
    module: Module,
    wasm_hash: String,
    tick_abi: TickAbi,
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

        let mut store = Store::new(&self.engine, BehaviorHostState::new(Default::default()));
        let mut linker = Linker::new(&self.engine);
        define_host_imports(&mut linker)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .context("instantiate behavior wasm for ABI validation")?;
        let tick_abi = validate_tick_abi(&instance, &mut store)?;

        Ok(CompiledBehavior {
            kind,
            module,
            wasm_hash,
            tick_abi,
        })
    }

    pub fn evaluate_compiled(
        &self,
        compiled: &CompiledBehavior,
        fuel: u64,
        input: BehaviorHostInput,
    ) -> Result<BehaviorEval> {
        let mut store = Store::new(&self.engine, BehaviorHostState::new(input));
        store.set_fuel(fuel).context("set behavior fuel")?;
        let mut linker = Linker::new(&self.engine);
        define_host_imports(&mut linker)?;
        let instance = linker
            .instantiate(&mut store, &compiled.module)
            .context("instantiate behavior wasm")?;

        let intent = match compiled.tick_abi {
            TickAbi::Void => {
                let tick = instance
                    .get_typed_func::<(), ()>(&mut store, "tick")
                    .context("load tick export")?;
                if tick.call(&mut store, ()).is_err() {
                    return Ok(over_budget_eval(&mut store, fuel, compiled));
                }
                store.data().intent.clone()
            }
            TickAbi::ActionCode => {
                let tick = instance
                    .get_typed_func::<(), i32>(&mut store, "tick")
                    .context("load tick export")?;
                let action_code = match tick.call(&mut store, ()) {
                    Ok(code) => code,
                    Err(_) => return Ok(over_budget_eval(&mut store, fuel, compiled)),
                };
                action_code_to_intent(compiled.kind, action_code)?
            }
        };
        let fuel_remaining = store.get_fuel().unwrap_or(0);

        Ok(BehaviorEval {
            intent,
            fuel_spent: fuel.saturating_sub(fuel_remaining),
            fuel_remaining,
            over_budget: false,
            wasm_hash: compiled.wasm_hash.clone(),
        })
    }
}

fn validate_tick_abi(instance: &Instance, store: &mut Store<BehaviorHostState>) -> Result<TickAbi> {
    if instance
        .get_typed_func::<(), ()>(&mut *store, "tick")
        .is_ok()
    {
        return Ok(TickAbi::Void);
    }
    if instance
        .get_typed_func::<(), i32>(&mut *store, "tick")
        .is_ok()
    {
        return Ok(TickAbi::ActionCode);
    }
    Err(anyhow!(
        "behavior must export tick() -> () or tick() -> i32"
    ))
}

fn over_budget_eval(
    store: &mut Store<BehaviorHostState>,
    fuel: u64,
    compiled: &CompiledBehavior,
) -> BehaviorEval {
    let fuel_remaining = store.get_fuel().unwrap_or(0);
    BehaviorEval {
        intent: BehaviorIntent::Noop,
        fuel_spent: fuel.saturating_sub(fuel_remaining),
        fuel_remaining,
        over_budget: true,
        wasm_hash: compiled.wasm_hash.clone(),
    }
}

fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    linker.func_wrap(
        "xac:drill",
        "output_blocked",
        |caller: Caller<'_, BehaviorHostState>| -> i32 {
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
            caller.data_mut().intent = BehaviorIntent::DrillDefault;
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_any",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            caller.data_mut().intent = BehaviorIntent::Router {
                preferred: Direction::all().to_vec(),
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_dir",
        |mut caller: Caller<'_, BehaviorHostState>, dir: i32| -> i32 {
            let dir = match dir {
                0 => Direction::North,
                1 => Direction::East,
                2 => Direction::South,
                3 => Direction::West,
                _ => return 0,
            };
            caller.data_mut().intent = BehaviorIntent::Router {
                preferred: vec![dir],
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "set_recipe",
        |mut caller: Caller<'_, BehaviorHostState>, recipe: i32| -> i32 {
            caller.data_mut().assembler_prefer_ammo = recipe == 1;
            1
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "can_produce",
        |caller: Caller<'_, BehaviorHostState>| -> i32 {
            if caller.data().input.can_produce {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "produce",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !caller.data().input.can_produce {
                return 0;
            }
            let prefer_ammo = caller.data().assembler_prefer_ammo;
            caller.data_mut().intent = BehaviorIntent::Assembler { prefer_ammo };
            1
        },
    )?;
    linker.func_wrap(
        "xac:turret",
        "ammo_count",
        |caller: Caller<'_, BehaviorHostState>| -> i32 { caller.data().input.ammo_count },
    )?;
    linker.func_wrap(
        "xac:turret",
        "attack_nearest",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
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
            if caller.data().input.ammo_count <= 0 {
                return 0;
            }
            let priority = if policy == 1 {
                vec![TargetRule::LowestHp, TargetRule::Nearest]
            } else {
                vec![TargetRule::Nearest]
            };
            caller.data_mut().intent = BehaviorIntent::Turret { priority };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "dispatch",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            caller.data_mut().intent = BehaviorIntent::DronePort;
            1
        },
    )?;
    Ok(())
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

pub fn wat_drill_mine() -> String {
    r#"(module
  (import "xac:drill" "output_blocked" (func $output_blocked (result i32)))
  (import "xac:drill" "mine" (func $mine (result i32)))
  (func (export "tick")
    (if (i32.eqz (call $output_blocked))
      (then
        (drop (call $mine))))))"#
        .to_string()
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
        let eval = runtime
            .evaluate_compiled(&compiled, 20, BehaviorHostInput::default())
            .unwrap();

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
            .compile_wat(BlockKind::Drill, &wat_const_action(30))
            .unwrap();

        let err = runtime
            .evaluate_compiled(&compiled, 20, BehaviorHostInput::default())
            .unwrap_err();
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

    #[test]
    fn host_imports_allow_drill_code_to_call_game_api() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(BlockKind::Drill, &wat_drill_mine())
            .unwrap();
        let eval = runtime
            .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
            .unwrap();

        assert!(matches!(eval.intent, BehaviorIntent::DrillDefault));
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
}
