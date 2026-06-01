use xac_core::{Block, BlockKind, Direction, EntityId, Inventory, Pos, TerrainKind, Tile};

use crate::{MAP_HEIGHT, MAP_WIDTH};

pub fn build_tiles() -> Vec<Tile> {
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
    let ore = ((pos.x - 20).pow(2) + (pos.y - 30).pow(2) < 42)
        || ((pos.x - 42).pow(2) + (pos.y - 25).pow(2) < 30)
        || ((pos.x - 30).pow(2) + (pos.y - 44).pow(2) < 28);
    if ore {
        TerrainKind::OrePatch
    } else {
        TerrainKind::Ground
    }
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
        progress: 0,
        status: "idle".to_string(),
    }
}

pub fn cpu_scaled_threshold(effective_cpu_rate: f32, base: u32) -> u32 {
    let speedup = (effective_cpu_rate / 8.0).clamp(0.1, 10.0);
    ((base as f32 / speedup).ceil() as u32).max(3)
}
