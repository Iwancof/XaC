use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use wasmtime::{Caller, Config, Instance, Linker, Module, Store};
use xac_core::{BlockKind, Direction, EnemyKind, ItemKind};

mod script;
use script::{
    compile_xac_script, is_wat_source, ATTACK_POLICY_ARMORED, ATTACK_POLICY_GRUNT,
    ATTACK_POLICY_LOWEST_HP, ATTACK_POLICY_NEAREST, ATTACK_POLICY_RUNNER,
    ATTACK_POLICY_WIRE_CUTTER,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BehaviorIntent {
    Noop,
    DrillDefault,
    Router { preferred: Vec<Direction> },
    Assembler { recipe: ItemKind },
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
    pub net_writes: Vec<NetStoreWrite>,
    pub fuel_spent: u64,
    pub fuel_remaining: u64,
    pub over_budget: bool,
    pub wasm_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetStoreWrite {
    pub key: i32,
    pub value: i32,
}

#[derive(Clone, Debug, Default)]
pub struct BehaviorHostInput {
    pub output_blocked: bool,
    pub can_produce: bool,
    pub assembler_can_produce: [bool; 2],
    pub ammo_count: i32,
    pub router_output_available: [bool; 4],
    pub net_i32: BTreeMap<i32, i32>,
    pub net_writable: bool,
}

#[derive(Clone, Debug)]
struct BehaviorHostState {
    input: BehaviorHostInput,
    intent: BehaviorIntent,
    assembler_recipe: ItemKind,
    net_writes: Vec<NetStoreWrite>,
    host_over_budget: bool,
}

impl BehaviorHostState {
    fn new(input: BehaviorHostInput) -> Self {
        Self {
            input,
            intent: BehaviorIntent::Noop,
            assembler_recipe: ItemKind::Ammo,
            net_writes: Vec::new(),
            host_over_budget: false,
        }
    }
}

mod host_cost {
    pub const FUEL_REMAINING: u64 = 0;
    pub const OUTPUT_BLOCKED: u64 = 1;
    pub const MINE: u64 = 2;
    pub const PUSH: u64 = 1;
    pub const OUTPUT_AVAILABLE: u64 = 1;
    pub const SET_RECIPE: u64 = 2;
    pub const CAN_PRODUCE: u64 = 1;
    pub const PRODUCE: u64 = 2;
    pub const AMMO_COUNT: u64 = 1;
    pub const ATTACK_NEAREST: u64 = 4;
    pub const ATTACK_BEST: u64 = 8;
    pub const DISPATCH: u64 = 5;
    pub const NET_GET_I32: u64 = 2;
    pub const NET_SET_I32: u64 = 4;
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
        let wat_source = compile_source_to_wat(kind, source)?;
        let wasm = wat::parse_str(&wat_source).context("parse behavior source as WAT")?;
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
        if store.data().host_over_budget {
            return Ok(over_budget_eval(&mut store, fuel, compiled));
        }
        let fuel_remaining = store.get_fuel().unwrap_or(0);

        Ok(BehaviorEval {
            intent,
            net_writes: store.data().net_writes.clone(),
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
        net_writes: Vec::new(),
        fuel_spent: fuel.saturating_sub(fuel_remaining),
        fuel_remaining,
        over_budget: true,
        wasm_hash: compiled.wasm_hash.clone(),
    }
}

fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    linker.func_wrap(
        "xac:common",
        "fuel_remaining",
        |mut caller: Caller<'_, BehaviorHostState>| -> i64 {
            charge_host(&mut caller, host_cost::FUEL_REMAINING);
            caller.get_fuel().unwrap_or(0) as i64
        },
    )?;
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
            caller.data_mut().intent = BehaviorIntent::DrillDefault;
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_any",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::PUSH) {
                return 0;
            }
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
            if !charge_host(&mut caller, host_cost::PUSH) {
                return 0;
            }
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            caller.data_mut().intent = BehaviorIntent::Router {
                preferred: vec![dir],
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "output_available",
        |mut caller: Caller<'_, BehaviorHostState>, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::OUTPUT_AVAILABLE) {
                return 0;
            }
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            if caller.data().input.router_output_available[direction_index(dir)] {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "set_recipe",
        |mut caller: Caller<'_, BehaviorHostState>, recipe: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::SET_RECIPE) {
                return 0;
            }
            let Some(recipe) = recipe_from_code(recipe) else {
                return 0;
            };
            caller.data_mut().assembler_recipe = recipe;
            1
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "can_produce",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::CAN_PRODUCE) {
                return 0;
            }
            let recipe = caller.data().assembler_recipe.clone();
            let can_progress = caller.data().input.assembler_can_produce[recipe_index(&recipe)]
                || caller.data().input.can_produce;
            if can_progress {
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
            if !charge_host(&mut caller, host_cost::PRODUCE) {
                return 0;
            }
            let recipe = caller.data().assembler_recipe.clone();
            let can_progress = caller.data().input.assembler_can_produce[recipe_index(&recipe)]
                || caller.data().input.can_produce;
            if !can_progress {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::Assembler { recipe };
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
            if !charge_host(&mut caller, host_cost::ATTACK_BEST) {
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
    linker.func_wrap(
        "xac:drone_port",
        "dispatch",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DISPATCH) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::DronePort;
            1
        },
    )?;
    linker.func_wrap(
        "xac:net",
        "store_get_i32",
        |mut caller: Caller<'_, BehaviorHostState>, key: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::NET_GET_I32) {
                return 0;
            }
            caller.data().input.net_i32.get(&key).copied().unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:net",
        "store_set_i32",
        |mut caller: Caller<'_, BehaviorHostState>, key: i32, value: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::NET_SET_I32) {
                return 0;
            }
            let data = caller.data_mut();
            if !data.input.net_writable {
                return 0;
            }
            data.input.net_i32.insert(key, value);
            data.net_writes.push(NetStoreWrite { key, value });
            1
        },
    )?;
    Ok(())
}

fn charge_host(caller: &mut Caller<'_, BehaviorHostState>, cost: u64) -> bool {
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

pub fn compile_source_to_wat(kind: BlockKind, source: &str) -> Result<String> {
    if is_wat_source(source) {
        Ok(source.to_string())
    } else {
        compile_xac_script(kind, source).map_err(|error| anyhow!("compile XaC script: {error}"))
    }
}

pub fn hash_wasm_source(source: &str) -> Result<String> {
    let wasm = wat::parse_str(source).context("parse behavior WAT")?;
    Ok(hash_bytes(&wasm))
}

pub fn hash_behavior_source(kind: BlockKind, source: &str) -> Result<String> {
    let wat_source = compile_source_to_wat(kind, source)?;
    let wasm = wat::parse_str(&wat_source).context("parse behavior source as WAT")?;
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
        20 if kind == BlockKind::Assembler => Ok(BehaviorIntent::Assembler {
            recipe: ItemKind::Plate,
        }),
        21 if kind == BlockKind::Assembler => Ok(BehaviorIntent::Assembler {
            recipe: ItemKind::Ammo,
        }),
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

fn attack_policy_to_rules(policy: i32) -> Option<Vec<TargetRule>> {
    if policy == 1 {
        return Some(vec![TargetRule::LowestHp, TargetRule::Nearest]);
    }
    if policy <= 0 {
        return Some(vec![TargetRule::Nearest]);
    }

    let mut encoded = policy as u32;
    let mut rules = Vec::new();
    while encoded > 0 {
        let code = (encoded & 0x0f) as i32;
        let rule = match code {
            ATTACK_POLICY_NEAREST => TargetRule::Nearest,
            ATTACK_POLICY_LOWEST_HP => TargetRule::LowestHp,
            ATTACK_POLICY_RUNNER => TargetRule::Kind(EnemyKind::Runner),
            ATTACK_POLICY_WIRE_CUTTER => TargetRule::Kind(EnemyKind::WireCutter),
            ATTACK_POLICY_ARMORED => TargetRule::Kind(EnemyKind::Armored),
            ATTACK_POLICY_GRUNT => TargetRule::Kind(EnemyKind::Grunt),
            _ => return None,
        };
        rules.push(rule);
        encoded >>= 4;
    }

    if rules.is_empty() {
        rules.push(TargetRule::Nearest);
    }
    Some(rules)
}

fn router_dir(dir: Direction) -> Result<BehaviorIntent> {
    Ok(BehaviorIntent::Router {
        preferred: vec![dir],
    })
}

fn direction_from_code(code: i32) -> Option<Direction> {
    match code {
        0 => Some(Direction::North),
        1 => Some(Direction::East),
        2 => Some(Direction::South),
        3 => Some(Direction::West),
        _ => None,
    }
}

fn direction_index(dir: Direction) -> usize {
    match dir {
        Direction::North => 0,
        Direction::East => 1,
        Direction::South => 2,
        Direction::West => 3,
    }
}

fn recipe_from_code(code: i32) -> Option<ItemKind> {
    match code {
        0 => Some(ItemKind::Plate),
        1 => Some(ItemKind::Ammo),
        _ => None,
    }
}

fn recipe_index(recipe: &ItemKind) -> usize {
    match recipe {
        ItemKind::Plate => 0,
        ItemKind::Ammo => 1,
        _ => 0,
    }
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
            BehaviorIntent::Assembler { recipe } if recipe == ItemKind::Ammo
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

    #[test]
    fn compiles_xac_script_to_host_imported_wasm() {
        let runtime = BehaviorRuntime::new().unwrap();
        let source = r#"
            # short player-facing drill code
            if output_blocked return
            mine
        "#;
        let wat = compile_source_to_wat(BlockKind::Drill, source).unwrap();
        assert!(wat.contains(r#"(import "xac:drill" "mine""#));
        assert!(wat.contains("(call $output_blocked)"));

        let compiled = runtime.compile_wat(BlockKind::Drill, source).unwrap();
        let eval = runtime
            .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
            .unwrap();
        assert!(matches!(eval.intent, BehaviorIntent::DrillDefault));

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
    fn xac_script_can_read_and_write_network_store() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(
                BlockKind::Turret,
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
                BlockKind::Turret,
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
    fn xac_script_can_gate_router_push_on_output_availability() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(BlockKind::Router, "if output_available east push east")
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
            BehaviorIntent::Router { preferred } if preferred == vec![Direction::East]
        ));
    }

    #[test]
    fn xac_script_can_branch_on_remaining_fuel() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(BlockKind::Drill, "if fuel_remaining > 12 mine")
            .unwrap();

        let eval = runtime
            .evaluate_compiled(&compiled, 8, BehaviorHostInput::default())
            .unwrap();
        assert!(matches!(eval.intent, BehaviorIntent::Noop));
        assert!(!eval.over_budget);

        let eval = runtime
            .evaluate_compiled(&compiled, 30, BehaviorHostInput::default())
            .unwrap();
        assert!(matches!(eval.intent, BehaviorIntent::DrillDefault));
        assert!(
            eval.fuel_spent >= host_cost::MINE,
            "host API mine should charge explicit fuel"
        );
    }

    #[test]
    fn host_api_cost_can_exhaust_behavior_budget() {
        let runtime = BehaviorRuntime::new().unwrap();
        let compiled = runtime
            .compile_wat(
                BlockKind::Turret,
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
        let err = compile_source_to_wat(BlockKind::Turret, "mine").unwrap_err();
        assert!(err.to_string().contains("only available to Drill"));
    }

    #[test]
    fn host_imports_map_logistics_production_and_combat_apis() {
        let runtime = BehaviorRuntime::new().unwrap();

        let router = runtime
            .compile_wat(
                BlockKind::Router,
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
            BehaviorIntent::Router { preferred } if preferred == vec![Direction::East]
        ));

        let assembler = runtime
            .compile_wat(
                BlockKind::Assembler,
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
            BehaviorIntent::Assembler { recipe } if recipe == ItemKind::Ammo
        ));

        let turret = runtime
            .compile_wat(
                BlockKind::Turret,
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
                BlockKind::DronePort,
                r#"(module
                  (import "xac:drone_port" "dispatch" (func $dispatch (result i32)))
                  (func (export "tick")
                    (drop (call $dispatch))))"#,
            )
            .unwrap();
        let eval = runtime
            .evaluate_compiled(&drone_port, 30, BehaviorHostInput::default())
            .unwrap();
        assert!(matches!(eval.intent, BehaviorIntent::DronePort));
    }
}
