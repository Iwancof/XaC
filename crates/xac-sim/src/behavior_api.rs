use anyhow::{anyhow, Result};
use xac_core::{
    BehaviorKind, BehaviorSource, BehaviorSummary, BuildResult, GameSnapshot, LogLevel,
};

use crate::behavior::{
    persist_project_behavior_index, persist_project_behavior_source, project_behavior_source_path,
    BehaviorPackage,
};
use crate::Simulation;

#[derive(Clone, Copy)]
enum BehaviorOwner {
    Block,
    Drone,
}

impl Simulation {
    pub fn open_behavior(&self, id: &str) -> Result<BehaviorSource> {
        let package = self
            .behaviors
            .get(id)
            .ok_or_else(|| anyhow!("unknown behavior: {id}"))?;
        Ok(BehaviorSource {
            summary: self.behavior_summary_with_usage(package),
            source: package.source.clone(),
        })
    }

    pub fn edit_builtin_copy(&mut self, entity_id: &str) -> Result<BehaviorSource> {
        let (owner, behavior_id, _) = self.behavior_owner(entity_id)?;
        let original = self
            .behaviors
            .get(&behavior_id)
            .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?
            .clone();

        if !original.summary.builtin {
            return self.open_behavior(&behavior_id);
        }

        let new_id = self.make_id("behavior");
        let display_name = format!("{} Copy", original.summary.display_name);
        let source_path = project_behavior_source_path(&self.config_root, &new_id)
            .to_string_lossy()
            .to_string();
        let summary = BehaviorSummary {
            id: new_id.clone(),
            display_name,
            base_kind: original.summary.base_kind,
            world: original.summary.world,
            builtin: false,
            used_by: 0,
            source_path,
            build_status: "copied".to_string(),
        };
        let package = BehaviorPackage {
            summary,
            source: original.source,
            wasm_hash: original.wasm_hash,
        };
        persist_project_behavior_source(&package)?;
        self.behaviors.insert(new_id.clone(), package);
        persist_project_behavior_index(&self.config_root, &self.behaviors)?;
        self.set_behavior_ref(owner, entity_id, new_id.clone(), None)?;
        self.log(
            LogLevel::Info,
            entity_id.to_string(),
            format!("created editable copy {new_id}"),
        );
        self.open_behavior(&new_id)
    }

    pub fn fork_behavior(&mut self, entity_id: &str) -> Result<BehaviorSource> {
        let (owner, behavior_id, _) = self.behavior_owner(entity_id)?;
        let original = self
            .behaviors
            .get(&behavior_id)
            .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?
            .clone();
        let new_id = self.make_id("behavior");
        let source_path = project_behavior_source_path(&self.config_root, &new_id)
            .to_string_lossy()
            .to_string();
        let summary = BehaviorSummary {
            id: new_id.clone(),
            display_name: format!("{} Fork", original.summary.display_name),
            base_kind: original.summary.base_kind,
            world: original.summary.world,
            builtin: false,
            used_by: 0,
            source_path,
            build_status: "forked".to_string(),
        };
        let package = BehaviorPackage {
            summary,
            source: original.source,
            wasm_hash: original.wasm_hash,
        };
        persist_project_behavior_source(&package)?;
        self.behaviors.insert(new_id.clone(), package);
        persist_project_behavior_index(&self.config_root, &self.behaviors)?;
        self.set_behavior_ref(owner, entity_id, new_id.clone(), None)?;
        self.log(
            LogLevel::Info,
            entity_id.to_string(),
            format!("forked behavior into {new_id}"),
        );
        self.open_behavior(&new_id)
    }

    pub fn assign_behavior(&mut self, entity_id: &str, behavior_id: &str) -> Result<GameSnapshot> {
        let (owner, _, expected_kind) = self.behavior_owner(entity_id)?;
        let behavior = self
            .behaviors
            .get(behavior_id)
            .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
        if behavior.summary.base_kind != expected_kind {
            return Err(anyhow!(
                "behavior {behavior_id} targets {:?}, but block is {:?}",
                behavior.summary.base_kind,
                expected_kind
            ));
        }

        let display_name = behavior.summary.display_name.clone();
        self.set_behavior_ref(
            owner,
            entity_id,
            behavior_id.to_string(),
            Some(&display_name),
        )?;
        self.log(
            LogLevel::Info,
            entity_id.to_string(),
            format!("assigned {display_name}"),
        );
        Ok(self.snapshot())
    }

    pub fn save_behavior(&mut self, behavior_id: &str, source: String) -> Result<BehaviorSource> {
        {
            let package = self
                .behaviors
                .get_mut(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            if package.summary.builtin {
                return Err(anyhow!(
                    "builtin presets are read-only; create a copy first"
                ));
            }
            package.source = source;
            package.wasm_hash = None;
            package.summary.build_status = "saved".to_string();
            persist_project_behavior_source(package)?;
        }
        self.compiled_behaviors.remove(behavior_id);
        persist_project_behavior_index(&self.config_root, &self.behaviors)?;
        self.log(
            LogLevel::Info,
            behavior_id.to_string(),
            "source saved".to_string(),
        );
        self.open_behavior(behavior_id)
    }

    pub fn build_behavior(&mut self, behavior_id: &str) -> Result<BuildResult> {
        let (kind, source) = {
            let package = self
                .behaviors
                .get(behavior_id)
                .ok_or_else(|| anyhow!("unknown behavior: {behavior_id}"))?;
            (package.summary.base_kind, package.source.clone())
        };
        match self.runtime.compile_wat(kind, &source) {
            Ok(compiled) => {
                let wasm_hash = Some(compiled.wasm_hash().to_string());
                self.compiled_behaviors
                    .insert(behavior_id.to_string(), compiled);
                if let Some(package) = self.behaviors.get_mut(behavior_id) {
                    package.wasm_hash = wasm_hash.clone();
                    package.summary.build_status = "built".to_string();
                }
                persist_project_behavior_index(&self.config_root, &self.behaviors)?;
                self.log(
                    LogLevel::Info,
                    behavior_id.to_string(),
                    "build ok; behavior source compiled to wasm".to_string(),
                );
                Ok(BuildResult {
                    behavior_id: behavior_id.to_string(),
                    success: true,
                    message: "behavior built and hot-reloaded".to_string(),
                    wasm_hash,
                })
            }
            Err(error) => {
                if let Some(package) = self.behaviors.get_mut(behavior_id) {
                    package.summary.build_status = "build failed".to_string();
                }
                persist_project_behavior_index(&self.config_root, &self.behaviors)?;
                self.log(LogLevel::Error, behavior_id.to_string(), error.to_string());
                Ok(BuildResult {
                    behavior_id: behavior_id.to_string(),
                    success: false,
                    message: error.to_string(),
                    wasm_hash: None,
                })
            }
        }
    }

    pub(crate) fn behavior_summary_with_usage(&self, package: &BehaviorPackage) -> BehaviorSummary {
        let mut summary = package.summary.clone();
        summary.used_by = self
            .blocks
            .values()
            .filter(|b| b.behavior_ref.as_ref() == Some(&summary.id))
            .count() as u32
            + self
                .drones
                .values()
                .filter(|d| d.behavior_ref.as_ref() == Some(&summary.id))
                .count() as u32;
        summary
    }

    fn behavior_owner(&self, entity_id: &str) -> Result<(BehaviorOwner, String, BehaviorKind)> {
        if let Some(block) = self.blocks.get(entity_id) {
            let expected_kind = BehaviorKind::from_block_kind(block.kind)
                .ok_or_else(|| anyhow!("selected block cannot run behavior"))?;
            let behavior_id = block
                .behavior_ref
                .clone()
                .ok_or_else(|| anyhow!("selected block has no behavior"))?;
            return Ok((BehaviorOwner::Block, behavior_id, expected_kind));
        }
        if let Some(drone) = self.drones.get(entity_id) {
            let behavior_id = drone
                .behavior_ref
                .clone()
                .ok_or_else(|| anyhow!("selected drone has no behavior"))?;
            return Ok((
                BehaviorOwner::Drone,
                behavior_id,
                BehaviorKind::CarrierDrone,
            ));
        }
        Err(anyhow!("unknown behavior owner: {entity_id}"))
    }

    fn set_behavior_ref(
        &mut self,
        owner: BehaviorOwner,
        entity_id: &str,
        behavior_id: String,
        display_name: Option<&str>,
    ) -> Result<()> {
        match owner {
            BehaviorOwner::Block => {
                let block = self
                    .blocks
                    .get_mut(entity_id)
                    .ok_or_else(|| anyhow!("unknown block: {entity_id}"))?;
                block.behavior_ref = Some(behavior_id);
                if let Some(display_name) = display_name {
                    block.status = format!("behavior: {display_name}");
                }
            }
            BehaviorOwner::Drone => {
                let drone = self
                    .drones
                    .get_mut(entity_id)
                    .ok_or_else(|| anyhow!("unknown drone: {entity_id}"))?;
                drone.behavior_ref = Some(behavior_id);
            }
        }
        Ok(())
    }
}
