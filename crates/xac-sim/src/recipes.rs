use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use xac_core::{Block, ItemKind};

const RECIPES_TOML: &str = include_str!("../../../assets/recipes.toml");

#[derive(Clone, Debug)]
pub(crate) struct Recipe {
    pub(crate) id: String,
    pub(crate) inputs: BTreeMap<ItemKind, u32>,
    pub(crate) outputs: BTreeMap<ItemKind, u32>,
    pub(crate) time_ticks: u32,
}

#[derive(Deserialize)]
struct RecipeFile {
    recipe: Vec<RecipeDef>,
}

#[derive(Deserialize)]
struct RecipeDef {
    id: String,
    inputs: BTreeMap<String, u32>,
    outputs: BTreeMap<String, u32>,
    time_ticks: u32,
}

static RECIPES: OnceLock<Vec<Recipe>> = OnceLock::new();

pub(crate) fn recipes() -> &'static [Recipe] {
    RECIPES.get_or_init(|| parse_recipes().expect("assets/recipes.toml must be valid"))
}

pub(crate) fn can_progress_recipe(block: &Block, goal: &str) -> bool {
    next_recipe_for_goal(block, Some(goal)).is_some()
}

pub(crate) fn can_progress_any_recipe(block: &Block) -> bool {
    next_recipe_for_goal(block, block.recipe.as_deref()).is_some()
}

pub(crate) fn next_recipe_for_goal(block: &Block, goal: Option<&str>) -> Option<&'static Recipe> {
    if let Some(goal) = goal {
        if let Some(recipe) = recipe_by_id(goal) {
            if can_run_recipe(block, recipe) {
                return Some(recipe);
            }

            for missing in missing_inputs(block, recipe) {
                if let Some(prerequisite) = recipes().iter().find(|candidate| {
                    recipe_outputs(candidate, &missing) && can_run_recipe(block, candidate)
                }) {
                    return Some(prerequisite);
                }
            }

            return None;
        }
    }

    recipes()
        .iter()
        .find(|candidate| can_run_recipe(block, candidate))
}

pub(crate) fn apply_recipe(block: &mut Block, recipe: &Recipe) -> bool {
    if !can_run_recipe(block, recipe) {
        return false;
    }

    for (item, amount) in &recipe.inputs {
        block.inventory.remove(item, *amount);
    }
    for (item, amount) in &recipe.outputs {
        block.inventory.add(item.clone(), *amount);
    }
    true
}

fn parse_recipes() -> Result<Vec<Recipe>> {
    let file: RecipeFile = toml::from_str(RECIPES_TOML).context("parse assets/recipes.toml")?;
    file.recipe
        .into_iter()
        .map(|recipe| {
            Ok(Recipe {
                id: recipe.id,
                inputs: parse_items(recipe.inputs)?,
                outputs: parse_items(recipe.outputs)?,
                time_ticks: recipe.time_ticks.max(1),
            })
        })
        .collect()
}

fn parse_items(raw: BTreeMap<String, u32>) -> Result<BTreeMap<ItemKind, u32>> {
    raw.into_iter()
        .map(|(id, amount)| {
            let item = ItemKind::from_id(&id).ok_or_else(|| anyhow!("unknown item id {id}"))?;
            Ok((item, amount))
        })
        .collect()
}

fn recipe_by_id(id: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|recipe| recipe.id == id)
}

fn missing_inputs(block: &Block, recipe: &Recipe) -> Vec<ItemKind> {
    recipe
        .inputs
        .iter()
        .filter(|(item, amount)| block.inventory.count(item) < **amount)
        .map(|(item, _)| item.clone())
        .collect()
}

fn recipe_outputs(recipe: &Recipe, item: &ItemKind) -> bool {
    recipe.outputs.get(item).copied().unwrap_or(0) > 0
}

fn can_run_recipe(block: &Block, recipe: &Recipe) -> bool {
    let has_inputs = recipe
        .inputs
        .iter()
        .all(|(item, amount)| block.inventory.count(item) >= *amount);
    if !has_inputs {
        return false;
    }

    let input_total: u32 = recipe.inputs.values().sum();
    let output_total: u32 = recipe.outputs.values().sum();
    block
        .inventory
        .total()
        .saturating_sub(input_total)
        .saturating_add(output_total)
        <= block.inventory.capacity
}
