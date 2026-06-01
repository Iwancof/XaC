use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use wasmtime::{Config, Instance, Linker, Module, Store};
#[cfg(test)]
use xac_core::Pos;
use xac_core::{BehaviorKind, Direction, EnemyKind, ItemKind};

mod host;
use host::define_host_imports;
#[cfg(test)]
use host::host_cost;

mod imports;
use imports::{allowed_host_import, allowed_worlds};
#[cfg(test)]
use imports::{ALL_BEHAVIOR_KINDS, HOST_IMPORT_SPECS};

mod script;
use script::{
    compile_xac_script, is_wat_source, ATTACK_POLICY_ARMORED, ATTACK_POLICY_GRUNT,
    ATTACK_POLICY_LOWEST_HP, ATTACK_POLICY_NEAREST, ATTACK_POLICY_RUNNER,
    ATTACK_POLICY_WIRE_CUTTER,
};
mod tiny;
use tiny::{compile_tiny_source, is_tiny_source};

mod types;
pub use types::{
    AssemblerCommand, BehaviorEval, BehaviorHostInput, BehaviorIntent, BehaviorLog, DrillCommand,
    DroneCommand, DronePortCommand, NetStoreDelete, NetStoreOp, NetStoreWrite, TargetRule,
};

#[derive(Clone, Debug)]
struct BehaviorHostState {
    input: BehaviorHostInput,
    intent: BehaviorIntent,
    assembler_recipe: ItemKind,
    net_ops: Vec<NetStoreOp>,
    logs: Vec<BehaviorLog>,
    host_over_budget: bool,
}

impl BehaviorHostState {
    fn new(input: BehaviorHostInput) -> Self {
        let assembler_recipe = input
            .assembler_current_recipe
            .clone()
            .unwrap_or(ItemKind::Ammo);
        Self {
            input,
            intent: BehaviorIntent::Noop,
            assembler_recipe,
            net_ops: Vec::new(),
            logs: Vec::new(),
            host_over_budget: false,
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
    kind: BehaviorKind,
    module: Module,
    wasm_hash: String,
    tick_abi: TickAbi,
}

impl CompiledBehavior {
    pub fn kind(&self) -> BehaviorKind {
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

    pub fn compile_wat(&self, kind: BehaviorKind, source: &str) -> Result<CompiledBehavior> {
        let wat_source = compile_source_to_wat(kind, source)?;
        let wasm = wat::parse_str(&wat_source).context("parse behavior source as WAT")?;
        let wasm_hash = hash_bytes(&wasm);
        let module = Module::new(&self.engine, &wasm).context("compile behavior wasm")?;
        validate_import_capabilities(kind, &module)?;

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
            net_ops: store.data().net_ops.clone(),
            logs: store.data().logs.clone(),
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

fn validate_import_capabilities(kind: BehaviorKind, module: &Module) -> Result<()> {
    for import in module.imports() {
        let import_module = import.module();
        let import_name = import.name();
        if !allowed_host_import(kind, import_module, import_name) {
            return Err(anyhow!(
                "{kind:?} behavior cannot import {import_module}/{import_name}; allowed worlds: {}",
                allowed_worlds(kind)
            ));
        }
    }
    Ok(())
}

fn over_budget_eval(
    store: &mut Store<BehaviorHostState>,
    fuel: u64,
    compiled: &CompiledBehavior,
) -> BehaviorEval {
    let fuel_remaining = store.get_fuel().unwrap_or(0);
    BehaviorEval {
        intent: BehaviorIntent::Noop,
        net_ops: Vec::new(),
        logs: Vec::new(),
        fuel_spent: fuel.saturating_sub(fuel_remaining),
        fuel_remaining,
        over_budget: true,
        wasm_hash: compiled.wasm_hash.clone(),
    }
}

pub fn compile_source_to_wat(kind: BehaviorKind, source: &str) -> Result<String> {
    if is_wat_source(source) {
        Ok(source.to_string())
    } else if is_tiny_source(source) {
        compile_tiny_source(kind, source).map_err(|error| anyhow!("compile XaC Tiny: {error}"))
    } else {
        compile_xac_script(kind, source).map_err(|error| anyhow!("compile XaC script: {error}"))
    }
}

pub fn hash_wasm_source(source: &str) -> Result<String> {
    let wasm = wat::parse_str(source).context("parse behavior WAT")?;
    Ok(hash_bytes(&wasm))
}

pub fn hash_behavior_source(kind: BehaviorKind, source: &str) -> Result<String> {
    let wat_source = compile_source_to_wat(kind, source)?;
    let wasm = wat::parse_str(&wat_source).context("parse behavior source as WAT")?;
    Ok(hash_bytes(&wasm))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn action_code_to_intent(kind: BehaviorKind, action_code: i32) -> Result<BehaviorIntent> {
    match action_code {
        0 => Ok(BehaviorIntent::Noop),
        1 if kind == BehaviorKind::Drill => Ok(BehaviorIntent::Drill {
            commands: vec![DrillCommand::Mine],
        }),
        10 if kind == BehaviorKind::Router => Ok(BehaviorIntent::Router {
            item: None,
            preferred: Direction::all().to_vec(),
        }),
        11 if kind == BehaviorKind::Router => router_dir(Direction::North),
        12 if kind == BehaviorKind::Router => router_dir(Direction::East),
        13 if kind == BehaviorKind::Router => router_dir(Direction::South),
        14 if kind == BehaviorKind::Router => router_dir(Direction::West),
        20 if kind == BehaviorKind::Assembler => Ok(BehaviorIntent::Assembler {
            commands: vec![
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Plate,
                },
                AssemblerCommand::Produce {
                    recipe: ItemKind::Plate,
                },
            ],
        }),
        21 if kind == BehaviorKind::Assembler => Ok(BehaviorIntent::Assembler {
            commands: vec![
                AssemblerCommand::SetRecipe {
                    recipe: ItemKind::Ammo,
                },
                AssemblerCommand::Produce {
                    recipe: ItemKind::Ammo,
                },
            ],
        }),
        30 if kind == BehaviorKind::Turret => Ok(BehaviorIntent::Turret {
            priority: vec![TargetRule::Nearest],
        }),
        31 if kind == BehaviorKind::Turret => Ok(BehaviorIntent::Turret {
            priority: vec![TargetRule::LowestHp, TargetRule::Nearest],
        }),
        40 if kind == BehaviorKind::DronePort => Ok(BehaviorIntent::DronePort {
            commands: vec![DronePortCommand::AutoDispatch],
        }),
        50 if kind == BehaviorKind::CarrierDrone => Ok(BehaviorIntent::CarrierDrone {
            command: DroneCommand::ReturnToPort,
        }),
        51 if kind == BehaviorKind::CarrierDrone => Ok(BehaviorIntent::CarrierDrone {
            command: DroneCommand::ClaimDeliveryJob,
        }),
        52 if kind == BehaviorKind::CarrierDrone => Ok(BehaviorIntent::CarrierDrone {
            command: DroneCommand::Deliver,
        }),
        53 if kind == BehaviorKind::CarrierDrone => Ok(BehaviorIntent::CarrierDrone {
            command: DroneCommand::Idle,
        }),
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
        item: None,
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

fn recipe_code(recipe: &ItemKind) -> i32 {
    match recipe {
        ItemKind::Plate => 0,
        ItemKind::Ammo => 1,
        _ => -1,
    }
}

fn recipe_index(recipe: &ItemKind) -> usize {
    match recipe {
        ItemKind::Plate => 0,
        ItemKind::Ammo => 1,
        _ => 0,
    }
}

fn item_from_code(code: i32) -> Option<ItemKind> {
    match code {
        0 => Some(ItemKind::Ore),
        1 => Some(ItemKind::Plate),
        2 => Some(ItemKind::Ammo),
        3 => Some(ItemKind::CpuPart),
        4 => Some(ItemKind::DronePart),
        _ => None,
    }
}

fn item_code(item: &ItemKind) -> i32 {
    match item {
        ItemKind::Ore => 0,
        ItemKind::Plate => 1,
        ItemKind::Ammo => 2,
        ItemKind::CpuPart => 3,
        ItemKind::DronePart => 4,
    }
}

fn enemy_kind_code(kind: &EnemyKind) -> i32 {
    match kind {
        EnemyKind::Grunt => 0,
        EnemyKind::Runner => 1,
        EnemyKind::Armored => 2,
        EnemyKind::WireCutter => 3,
    }
}

fn dropoff_tag_from_code(code: i32) -> Option<&'static str> {
    match code {
        0 => Some("frontline"),
        _ => None,
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
mod tests;
