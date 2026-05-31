use std::collections::BTreeMap;

use xac_core::{Block, BlockKind, EntityId, Pos, WorldPos};

pub fn footprint_positions(kind: BlockKind, pos: Pos) -> Vec<Pos> {
    let (width, height) = kind.footprint_size();
    let mut positions = Vec::with_capacity((width * height) as usize);
    for y in pos.y..pos.y + height {
        for x in pos.x..pos.x + width {
            positions.push(Pos { x, y });
        }
    }
    positions
}

pub fn block_center(block: &Block) -> WorldPos {
    let (width, height) = block.kind.footprint_size();
    WorldPos {
        x: block.pos.x as f32 + width as f32 / 2.0,
        y: block.pos.y as f32 + height as f32 / 2.0,
    }
}

pub fn closest_point_on_block(origin: WorldPos, block: &Block) -> WorldPos {
    let (width, height) = block.kind.footprint_size();
    let min_x = block.pos.x as f32;
    let min_y = block.pos.y as f32;
    let max_x = min_x + width as f32;
    let max_y = min_y + height as f32;
    WorldPos {
        x: origin.x.clamp(min_x, max_x),
        y: origin.y.clamp(min_y, max_y),
    }
}

pub fn nearest_block_target(
    blocks: &BTreeMap<EntityId, Block>,
    origin: WorldPos,
    predicate: impl Fn(BlockKind) -> bool,
) -> Option<(EntityId, WorldPos)> {
    blocks
        .values()
        .filter(|block| predicate(block.kind))
        .min_by(|a, b| {
            origin
                .distance(closest_point_on_block(origin, a))
                .total_cmp(&origin.distance(closest_point_on_block(origin, b)))
        })
        .map(|block| (block.id.clone(), closest_point_on_block(origin, block)))
}
