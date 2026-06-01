use anyhow::{anyhow, Result};
use xac_core::{BehaviorKind, Direction, LogLevel};
use xac_wasm::{AssemblerCommand, BehaviorIntent, CompiledBehavior, DrillCommand};

use crate::behavior::{persist_compiled_behavior_cache, persist_project_behavior_index};
use crate::cpu::FuelPolicy;
use crate::Simulation;

const BLOCK_BEHAVIOR_FUEL_POLICY: FuelPolicy = FuelPolicy {
    min_invocation_fuel: 40,
    max_bank_seconds: 8.0,
};

impl Simulation {
    pub(crate) fn run_programmable_behaviors(&mut self) {
        let ids: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.kind.is_programmable() && b.active)
            .map(|b| b.id.clone())
            .collect();

        for id in ids {
            let (kind, behavior_ref, cpu_rate) = match self.blocks.get(&id) {
                Some(block) => (
                    BehaviorKind::from_block_kind(block.kind)
                        .expect("programmable blocks have behavior kinds"),
                    block.behavior_ref.clone(),
                    block.effective_cpu_rate,
                ),
                None => continue,
            };
            let Some(behavior_ref) = behavior_ref else {
                continue;
            };
            let fuel = self.grant_fuel_bank(&id, cpu_rate, BLOCK_BEHAVIOR_FUEL_POLICY);
            if fuel == 0 {
                continue;
            }
            let compiled = match self.compiled_behavior(&behavior_ref, kind) {
                Ok(compiled) => compiled,
                Err(error) => {
                    if let Some(block) = self.blocks.get_mut(&id) {
                        block.status = "runtime error".to_string();
                    }
                    self.log(LogLevel::Error, id, error.to_string());
                    continue;
                }
            };
            let host_input = self.behavior_host_input(&id);

            match self.runtime.evaluate_compiled(&compiled, fuel, host_input) {
                Ok(eval) => {
                    let runtime_error = eval.runtime_error.clone();
                    let wasm_hash = eval.wasm_hash.clone();
                    self.record_block_behavior_runtime(&id, fuel, &eval);
                    self.spend_fuel_bank(&id, eval.fuel_spent);
                    if let Some(error) = runtime_error {
                        if let Some(package) = self.behaviors.get_mut(&behavior_ref) {
                            package.wasm_hash = Some(wasm_hash);
                        }
                        if let Some(block) = self.blocks.get_mut(&id) {
                            block.status = "runtime error".to_string();
                        }
                        self.log(LogLevel::Error, id, error);
                        continue;
                    }
                    if eval.over_budget {
                        if let Some(block) = self.blocks.get_mut(&id) {
                            block.status = "over_budget".to_string();
                        }
                        self.log(
                            LogLevel::Warn,
                            id.clone(),
                            format!("over_budget with {fuel} fuel"),
                        );
                        continue;
                    }
                    if let Some(package) = self.behaviors.get_mut(&behavior_ref) {
                        package.wasm_hash = Some(wasm_hash);
                    }
                    self.apply_net_ops(&id, eval.net_ops);
                    self.apply_behavior_logs(&id, eval.logs);
                    self.apply_behavior_intent(&id, eval.intent);
                }
                Err(error) => {
                    let message = error.to_string();
                    self.spend_fuel_bank(&id, fuel);
                    self.record_block_behavior_runtime_error(
                        &id,
                        fuel,
                        fuel,
                        0,
                        Some(compiled.wasm_hash().to_string()),
                        message.clone(),
                    );
                    if let Some(block) = self.blocks.get_mut(&id) {
                        block.status = "runtime error".to_string();
                    }
                    self.log(LogLevel::Error, id, message);
                }
            }
        }
    }

    pub(crate) fn compiled_behavior(
        &mut self,
        behavior_id: &str,
        expected_kind: BehaviorKind,
    ) -> Result<CompiledBehavior> {
        if let Some(compiled) = self.compiled_behaviors.get(behavior_id) {
            return Ok(compiled.clone());
        }

        let (kind, source) = {
            let package = self
                .behaviors
                .get(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            (package.summary.base_kind, package.source.clone())
        };
        if kind != expected_kind {
            return Err(anyhow!(
                "behavior {behavior_id} targets {kind:?}, but block is {expected_kind:?}"
            ));
        }

        let compiled = self.runtime.compile_wat(kind, &source)?;
        {
            let package = self
                .behaviors
                .get(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            persist_compiled_behavior_cache(&self.config_root, package, &compiled)?;
        }
        let wasm_hash = compiled.wasm_hash().to_string();
        let mut persist_index = false;
        if let Some(package) = self.behaviors.get_mut(behavior_id) {
            package.wasm_hash = Some(wasm_hash);
            if package.summary.build_status != "built" {
                package.summary.build_status = "compiled".to_string();
            }
            persist_index = !package.summary.builtin;
        }
        if persist_index {
            persist_project_behavior_index(&self.config_root, &self.behaviors)?;
        }
        self.compiled_behaviors
            .insert(behavior_id.to_string(), compiled.clone());
        Ok(compiled)
    }

    fn apply_behavior_intent(&mut self, block_id: &str, intent: BehaviorIntent) {
        match intent {
            BehaviorIntent::Noop => {}
            BehaviorIntent::Drill { commands } => {
                for command in commands {
                    match command {
                        DrillCommand::Mine => self.run_drill(block_id),
                        DrillCommand::Output { item } => {
                            let Some(dir) = self.blocks.get(block_id).map(|block| block.dir) else {
                                continue;
                            };
                            self.transfer_item_from(block_id, dir, &item, 1);
                        }
                    }
                }
            }
            BehaviorIntent::Router { item, preferred } => {
                let dirs = if preferred.is_empty() {
                    Direction::all().to_vec()
                } else {
                    preferred
                };
                for dir in dirs {
                    let moved = if let Some(item) = item.as_ref() {
                        self.transfer_item_from(block_id, dir, item, 1)
                    } else {
                        self.transfer_from(block_id, dir, 1)
                    };
                    if moved {
                        break;
                    }
                }
            }
            BehaviorIntent::Assembler { commands } => {
                for command in commands {
                    match command {
                        AssemblerCommand::SetRecipe { recipe } => {
                            let recipe_id = recipe.as_str().to_string();
                            if let Some(block) = self.blocks.get_mut(block_id) {
                                block.recipe = Some(recipe_id.clone());
                                block.status = format!("recipe: {recipe_id} priority");
                            }
                        }
                        AssemblerCommand::Produce { recipe } => {
                            let recipe_id = recipe.as_str().to_string();
                            if let Some(block) = self.blocks.get_mut(block_id) {
                                block.recipe = Some(recipe_id);
                            }
                            self.run_assembler(block_id);
                        }
                    }
                }
            }
            BehaviorIntent::Turret { priority } => self.run_turret_once(block_id, &priority),
            BehaviorIntent::TurretScanIndex { index } => {
                self.run_turret_scan_index(block_id, index);
            }
            BehaviorIntent::DronePort { commands } => {
                self.apply_drone_port_commands(block_id, commands);
            }
            BehaviorIntent::CarrierDrone { .. } => {}
        }
    }
}
