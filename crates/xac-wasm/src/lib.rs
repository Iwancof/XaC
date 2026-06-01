use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use wasmtime::{Caller, Config, Extern, Instance, Linker, Module, Store};
use xac_core::{BehaviorKind, Direction, EnemyKind, ItemKind, Pos};

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

mod host_cost {
    pub const LOG_BASE: u64 = 1;
    pub const FUEL_REMAINING: u64 = 0;
    pub const OUTPUT_BLOCKED: u64 = 1;
    pub const MINE: u64 = 2;
    pub const DRILL_OUTPUT: u64 = 1;
    pub const ORE_KIND: u64 = 1;
    pub const PUSH: u64 = 1;
    pub const PUSH_ITEM: u64 = 2;
    pub const OUTPUT_AVAILABLE: u64 = 1;
    pub const OUTPUT_ITEM_AVAILABLE: u64 = 2;
    pub const SET_RECIPE: u64 = 2;
    pub const CURRENT_RECIPE: u64 = 1;
    pub const CAN_PRODUCE: u64 = 1;
    pub const PRODUCE: u64 = 2;
    pub const ASSEMBLER_COUNT: u64 = 1;
    pub const AMMO_COUNT: u64 = 1;
    pub const SCAN_ENEMIES_BASE: u64 = 5;
    pub const CAN_ATTACK: u64 = 1;
    pub const ATTACK: u64 = 2;
    pub const ATTACK_NEAREST: u64 = 4;
    pub const ATTACK_BEST: u64 = 8;
    pub const DISPATCH: u64 = 5;
    pub const DRONE_PORT_STOCK: u64 = 2;
    pub const DRONE_PORT_CHARGE: u64 = 2;
    pub const DRONE_PORT_DOCKED_COUNT: u64 = 1;
    pub const DRONE_PORT_PENDING_JOB_COUNT: u64 = 1;
    pub const DRONE_PORT_CREATE_JOB: u64 = 6;
    pub const DRONE_PORT_DISPATCH_IDLE: u64 = 5;
    pub const DRONE_SENSOR: u64 = 1;
    pub const DRONE_JOB: u64 = 5;
    pub const DRONE_ACTION: u64 = 2;
    pub const DRONE_MOVE_TO: u64 = 2;
    pub const DRONE_CARGO: u64 = 1;
    pub const STOCK_COUNT: u64 = 2;
    pub const STOCK_CAPACITY: u64 = 2;
    pub const HAS_SPACE: u64 = 2;
    pub const NET_GET_I32: u64 = 2;
    pub const NET_SET_I32: u64 = 4;
    pub const NET_DELETE_I32: u64 = 2;
}

const MAX_LOG_MESSAGE_BYTES: usize = 256;

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

fn allowed_host_import(kind: BehaviorKind, module: &str, name: &str) -> bool {
    allowed_common_import(module, name) || allowed_kind_import(kind, module, name)
}

fn allowed_common_import(module: &str, name: &str) -> bool {
    match module {
        "xac:common" => matches!(
            name,
            "log" | "fuel_remaining" | "stock_count" | "stock_capacity" | "has_space"
        ),
        "xac:net" => matches!(name, "store_get_i32" | "store_set_i32" | "store_delete_i32"),
        _ => false,
    }
}

fn allowed_kind_import(kind: BehaviorKind, module: &str, name: &str) -> bool {
    match kind {
        BehaviorKind::Drill => {
            module == "xac:drill"
                && matches!(name, "output_blocked" | "mine" | "output" | "ore_kind")
        }
        BehaviorKind::Router => {
            module == "xac:router"
                && matches!(
                    name,
                    "push_any"
                        | "push_dir"
                        | "push_item_dir"
                        | "output_available"
                        | "output_item_available"
                )
        }
        BehaviorKind::Assembler => {
            module == "xac:assembler"
                && matches!(
                    name,
                    "set_recipe"
                        | "current_recipe"
                        | "can_produce"
                        | "input_count"
                        | "output_count"
                        | "produce"
                )
        }
        BehaviorKind::Turret => {
            module == "xac:turret"
                && matches!(
                    name,
                    "scan_enemies"
                        | "can_attack"
                        | "attack"
                        | "ammo_count"
                        | "attack_nearest"
                        | "attack_best"
                )
        }
        BehaviorKind::DronePort => {
            module == "xac:drone_port"
                && matches!(
                    name,
                    "dispatch"
                        | "stock_count"
                        | "charge_docked_drones"
                        | "docked_drone_count"
                        | "pending_job_count"
                        | "create_delivery_job"
                        | "dispatch_idle_drones"
                )
        }
        BehaviorKind::CarrierDrone => {
            module == "xac:drone"
                && matches!(
                    name,
                    "battery_percent"
                        | "logic_fuel_remaining"
                        | "has_job"
                        | "has_pending_job"
                        | "return_to_port"
                        | "claim_delivery_job"
                        | "deliver"
                        | "move_to"
                        | "load"
                        | "unload"
                        | "cargo_count"
                        | "battery_ratio"
                        | "idle"
                )
        }
    }
}

fn allowed_worlds(kind: BehaviorKind) -> &'static str {
    match kind {
        BehaviorKind::Drill => {
            "xac:common, xac:net, xac:drill(output_blocked, mine, output, ore_kind)"
        }
        BehaviorKind::Router => "xac:common, xac:net, xac:router",
        BehaviorKind::Assembler => "xac:common, xac:net, xac:assembler",
        BehaviorKind::Turret => "xac:common, xac:net, xac:turret",
        BehaviorKind::DronePort => "xac:common, xac:net, xac:drone_port",
        BehaviorKind::CarrierDrone => "xac:common, xac:net, xac:drone",
    }
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

fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    linker.func_wrap(
        "xac:common",
        "log",
        |mut caller: Caller<'_, BehaviorHostState>, ptr: i32, len: i32| -> i32 {
            let Ok(len) = usize::try_from(len) else {
                return 0;
            };
            let Ok(ptr) = usize::try_from(ptr) else {
                return 0;
            };
            if len > MAX_LOG_MESSAGE_BYTES {
                return 0;
            }
            let cost = host_cost::LOG_BASE + (len as u64 / 32);
            if !charge_host(&mut caller, cost) {
                return 0;
            }
            let Some(message) = read_guest_string(&mut caller, ptr, len) else {
                return 0;
            };
            caller.data_mut().logs.push(BehaviorLog { message });
            1
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "fuel_remaining",
        |mut caller: Caller<'_, BehaviorHostState>| -> i64 {
            charge_host(&mut caller, host_cost::FUEL_REMAINING);
            caller.get_fuel().unwrap_or(0) as i64
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "stock_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::STOCK_COUNT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .network_stock_counts
                .get(&item)
                .or_else(|| caller.data().input.drone_port_stock_counts.get(&item))
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "stock_capacity",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::STOCK_CAPACITY) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .network_stock_capacity
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:common",
        "has_space",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, amount: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::HAS_SPACE) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 1;
            }
            let space = caller
                .data()
                .input
                .network_stock_space
                .get(&item)
                .copied()
                .unwrap_or(0);
            if space >= i32::try_from(amount).unwrap_or(i32::MAX) {
                1
            } else {
                0
            }
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
            push_drill_command(caller.data_mut(), DrillCommand::Mine);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drill",
        "output",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRILL_OUTPUT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            push_drill_command(caller.data_mut(), DrillCommand::Output { item });
            1
        },
    )?;
    linker.func_wrap(
        "xac:drill",
        "ore_kind",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::ORE_KIND) {
                return -1;
            }
            caller
                .data()
                .input
                .drill_ore_kind
                .as_ref()
                .map(item_code)
                .unwrap_or(-1)
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
                item: None,
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
                item: None,
                preferred: vec![dir],
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:router",
        "push_item_dir",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::PUSH_ITEM) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            caller.data_mut().intent = BehaviorIntent::Router {
                item: Some(item),
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
        "xac:router",
        "output_item_available",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, dir: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::OUTPUT_ITEM_AVAILABLE) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Some(dir) = direction_from_code(dir) else {
                return 0;
            };
            let available = caller
                .data()
                .input
                .router_item_output_available
                .get(&item)
                .map(|by_dir| by_dir[direction_index(dir)])
                .unwrap_or(false);
            if available {
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
            caller.data_mut().assembler_recipe = recipe.clone();
            push_assembler_command(caller.data_mut(), AssemblerCommand::SetRecipe { recipe });
            1
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "current_recipe",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::CURRENT_RECIPE) {
                return -1;
            }
            caller
                .data()
                .input
                .assembler_current_recipe
                .as_ref()
                .map(recipe_code)
                .unwrap_or(-1)
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
        "input_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ASSEMBLER_COUNT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .assembler_input_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:assembler",
        "output_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::ASSEMBLER_COUNT) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .assembler_output_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
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
            push_assembler_command(caller.data_mut(), AssemblerCommand::Produce { recipe });
            1
        },
    )?;
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
            push_drone_port_command(caller.data_mut(), DronePortCommand::AutoDispatch);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "stock_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_STOCK) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .network_stock_counts
                .get(&item)
                .or_else(|| caller.data().input.drone_port_stock_counts.get(&item))
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "charge_docked_drones",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_CHARGE) {
                return 0;
            }
            push_drone_port_command(caller.data_mut(), DronePortCommand::ChargeDockedDrones);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "docked_drone_count",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_DOCKED_COUNT) {
                return 0;
            }
            caller.data().input.drone_port_docked_drone_count
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "pending_job_count",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_PENDING_JOB_COUNT) {
                return 0;
            }
            caller.data().input.drone_port_pending_job_count
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "create_delivery_job",
        |mut caller: Caller<'_, BehaviorHostState>,
         item: i32,
         amount: i32,
         dropoff_tag: i32|
         -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_CREATE_JOB) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Some(dropoff_tag) = dropoff_tag_from_code(dropoff_tag) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 0;
            }
            if caller
                .data()
                .input
                .network_stock_counts
                .get(&item)
                .or_else(|| caller.data().input.drone_port_stock_counts.get(&item))
                .copied()
                .unwrap_or(0)
                < i32::try_from(amount).unwrap_or(i32::MAX)
            {
                return 0;
            }
            push_drone_port_command(
                caller.data_mut(),
                DronePortCommand::CreateDeliveryJob {
                    item,
                    amount,
                    dropoff_tag: dropoff_tag.to_string(),
                },
            );
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone_port",
        "dispatch_idle_drones",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_PORT_DISPATCH_IDLE) {
                return 0;
            }
            push_drone_port_command(caller.data_mut(), DronePortCommand::DispatchIdleDrones);
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "battery_percent",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0;
            }
            caller.data().input.drone_battery_percent
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "battery_ratio",
        |mut caller: Caller<'_, BehaviorHostState>| -> f32 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0.0;
            }
            caller.data().input.drone_battery_percent.clamp(0, 100) as f32 / 100.0
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "logic_fuel_remaining",
        |mut caller: Caller<'_, BehaviorHostState>| -> i64 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0;
            }
            caller.data().input.drone_logic_fuel as i64
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "has_job",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_SENSOR) {
                return 0;
            }
            if caller.data().input.drone_has_job {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "has_pending_job",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_JOB) {
                return 0;
            }
            if caller.data().input.drone_has_pending_job {
                1
            } else {
                0
            }
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "return_to_port",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::ReturnToPort,
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "claim_delivery_job",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_JOB) {
                return 0;
            }
            if !caller.data().input.drone_has_pending_job {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::ClaimDeliveryJob,
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "deliver",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            if !caller.data().input.drone_has_job {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Deliver,
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "move_to",
        |mut caller: Caller<'_, BehaviorHostState>, x: i32, y: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_MOVE_TO) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::MoveTo { pos: Pos { x, y } },
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "load",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, amount: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Load { item, amount },
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "unload",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32, amount: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            let Ok(amount) = u32::try_from(amount) else {
                return 0;
            };
            if amount == 0 {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Unload { item, amount },
            };
            1
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "cargo_count",
        |mut caller: Caller<'_, BehaviorHostState>, item: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_CARGO) {
                return 0;
            }
            let Some(item) = item_from_code(item) else {
                return 0;
            };
            caller
                .data()
                .input
                .drone_cargo_counts
                .get(&item)
                .copied()
                .unwrap_or(0)
        },
    )?;
    linker.func_wrap(
        "xac:drone",
        "idle",
        |mut caller: Caller<'_, BehaviorHostState>| -> i32 {
            if !charge_host(&mut caller, host_cost::DRONE_ACTION) {
                return 0;
            }
            caller.data_mut().intent = BehaviorIntent::CarrierDrone {
                command: DroneCommand::Idle,
            };
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
            data.net_ops
                .push(NetStoreOp::Set(NetStoreWrite { key, value }));
            1
        },
    )?;
    linker.func_wrap(
        "xac:net",
        "store_delete_i32",
        |mut caller: Caller<'_, BehaviorHostState>, key: i32| -> i32 {
            if !charge_host(&mut caller, host_cost::NET_DELETE_I32) {
                return 0;
            }
            let data = caller.data_mut();
            if !data.input.net_writable {
                return 0;
            }
            data.input.net_i32.remove(&key);
            data.net_ops
                .push(NetStoreOp::Delete(NetStoreDelete { key }));
            1
        },
    )?;
    Ok(())
}

fn turret_can_attack_scan_index(state: &BehaviorHostState, index: i32) -> bool {
    state.input.ammo_count > 0 && index >= 0 && index < state.input.turret_visible_enemy_count
}

fn push_drill_command(state: &mut BehaviorHostState, command: DrillCommand) {
    match &mut state.intent {
        BehaviorIntent::Drill { commands } => commands.push(command),
        _ => {
            state.intent = BehaviorIntent::Drill {
                commands: vec![command],
            };
        }
    }
}

fn push_assembler_command(state: &mut BehaviorHostState, command: AssemblerCommand) {
    match &mut state.intent {
        BehaviorIntent::Assembler { commands } => commands.push(command),
        _ => {
            state.intent = BehaviorIntent::Assembler {
                commands: vec![command],
            };
        }
    }
}

fn push_drone_port_command(state: &mut BehaviorHostState, command: DronePortCommand) {
    match &mut state.intent {
        BehaviorIntent::DronePort { commands } => commands.push(command),
        _ => {
            state.intent = BehaviorIntent::DronePort {
                commands: vec![command],
            };
        }
    }
}

fn read_guest_string(
    caller: &mut Caller<'_, BehaviorHostState>,
    ptr: usize,
    len: usize,
) -> Option<String> {
    let memory = match caller.get_export("memory")? {
        Extern::Memory(memory) => memory,
        _ => return None,
    };
    let mut bytes = vec![0_u8; len];
    memory.read(caller, ptr, &mut bytes).ok()?;
    String::from_utf8(bytes).ok()
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
