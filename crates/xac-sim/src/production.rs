use xac_core::{BlockKind, ItemKind, TerrainKind};

use crate::block_defs::cpu_scaled_threshold;
use crate::recipes::{
    apply_recipe, can_progress_any_recipe, can_progress_recipe, next_recipe_for_goal,
};
use crate::Simulation;

impl Simulation {
    pub(crate) fn run_drill(&mut self, block_id: &str) {
        let mine_ready = self
            .blocks
            .get(block_id)
            .map(|b| {
                self.tile_at(b.pos)
                    .is_some_and(|t| t.terrain == TerrainKind::OrePatch)
            })
            .unwrap_or(false);
        if !mine_ready {
            return;
        }
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.progress += 1;
            let threshold = cpu_scaled_threshold(block.effective_cpu_rate, 30);
            if block.progress >= threshold && block.inventory.has_space(1) {
                block.progress = 0;
                block.inventory.add(ItemKind::Ore, 1);
                block.status = "mined ore".to_string();
            }
        }
    }

    pub(crate) fn run_assembler(&mut self, block_id: &str) {
        let recipe = {
            let Some(block) = self.blocks.get(block_id) else {
                return;
            };
            if block.kind != BlockKind::Assembler {
                return;
            }
            next_recipe_for_goal(block, block.recipe.as_deref()).cloned()
        };

        let Some(recipe) = recipe else {
            if let Some(block) = self.blocks.get_mut(block_id) {
                block.progress = 0;
                let goal = block.recipe.as_deref().unwrap_or("any");
                block.status = format!("waiting for {goal} inputs");
            }
            return;
        };

        let Some(block) = self.blocks.get_mut(block_id) else {
            return;
        };
        block.progress += 1;
        let threshold = cpu_scaled_threshold(block.effective_cpu_rate, recipe.time_ticks);
        if block.progress < threshold {
            return;
        }
        block.progress = 0;
        if apply_recipe(block, &recipe) {
            block.status = format!("produced {}", recipe.id);
        }
    }

    pub(crate) fn can_produce(&self, block_id: &str) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
            return false;
        };
        if block.kind != BlockKind::Assembler {
            return false;
        }
        can_progress_any_recipe(block)
    }

    pub(crate) fn can_progress_recipe(&self, block_id: &str, recipe_id: &str) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
            return false;
        };
        if block.kind != BlockKind::Assembler {
            return false;
        }
        can_progress_recipe(block, recipe_id)
    }
}
