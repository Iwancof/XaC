use serde::Deserialize;
use std::sync::OnceLock;
use xac_core::{Block, BlockKind, Direction, EntityId, Inventory, Pos, TerrainKind, Tile};

use crate::{MAP_HEIGHT, MAP_WIDTH};

pub fn build_tiles() -> Vec<Tile> {
    let seed = map_seed();
    debug_assert_eq!(seed.width, MAP_WIDTH);
    debug_assert_eq!(seed.height, MAP_HEIGHT);

    let mut tiles = Vec::with_capacity((MAP_WIDTH * MAP_HEIGHT) as usize);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tiles.push(Tile {
                pos: Pos { x, y },
                terrain: terrain_at(Pos { x, y }),
                buildable: true,
                enemy_passable: true,
                block_id: None,
            });
        }
    }
    tiles
}

pub fn terrain_at(pos: Pos) -> TerrainKind {
    if map_seed()
        .ore_patches
        .iter()
        .any(|patch| patch.contains(pos))
    {
        TerrainKind::OrePatch
    } else {
        TerrainKind::Ground
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapSeed {
    width: i32,
    height: i32,
    ore_patches: Vec<OrePatchSeed>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrePatchSeed {
    center: [i32; 2],
    radius_squared: i32,
}

impl OrePatchSeed {
    fn contains(&self, pos: Pos) -> bool {
        let dx = pos.x - self.center[0];
        let dy = pos.y - self.center[1];
        dx.pow(2) + dy.pow(2) < self.radius_squared
    }
}

fn map_seed() -> &'static MapSeed {
    static MAP_SEED: OnceLock<MapSeed> = OnceLock::new();
    MAP_SEED.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/map_seed.json"))
            .expect("assets/map_seed.json must be valid")
    })
}

#[cfg(test)]
pub(crate) fn assert_map_seed_matches_dimensions() {
    let seed = map_seed();
    assert_eq!(seed.width, MAP_WIDTH);
    assert_eq!(seed.height, MAP_HEIGHT);
}

pub fn make_block(id: EntityId, kind: BlockKind, pos: Pos, dir: Direction) -> Block {
    Block {
        id,
        kind,
        pos,
        dir,
        hp: kind.max_hp(),
        inventory: Inventory::with_capacity(kind.inventory_capacity()),
        recipe: None,
        behavior_ref: None,
        tags: Vec::new(),
        active: kind.is_programmable(),
        network_id: None,
        effective_cpu_rate: kind.local_cpu_rate(),
        fuel_bank: 0.0,
        behavior_runtime: None,
        progress: 0,
        target_id: None,
        status: "idle".to_string(),
    }
}
