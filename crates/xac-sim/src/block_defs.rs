use xac_core::{
    Block, BlockKind, Direction, EntityId, Inventory, ItemKind, Pos, TerrainKind, Tile,
};

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
        hp: block_hp(kind),
        inventory: Inventory::with_capacity(inventory_capacity(kind)),
        behavior_ref: None,
        tags: Vec::new(),
        active: kind.is_programmable(),
        network_id: None,
        effective_cpu_rate: kind.local_cpu_rate(),
        progress: 0,
        status: "idle".to_string(),
    }
}

pub fn kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Core => "core",
        BlockKind::Wire => "wire",
        BlockKind::CpuNode => "cpu_node",
        BlockKind::Drill => "drill",
        BlockKind::Conveyor => "conveyor",
        BlockKind::Router => "router",
        BlockKind::Storage => "storage",
        BlockKind::Assembler => "assembler",
        BlockKind::Turret => "turret",
        BlockKind::DronePort => "drone_port",
    }
}

pub fn default_behavior_for(kind: BlockKind) -> Option<&'static str> {
    match kind {
        BlockKind::Drill => Some("builtin.drill.basic"),
        BlockKind::Router => Some("builtin.router.basic"),
        BlockKind::Assembler => Some("builtin.assembler.basic"),
        BlockKind::Turret => Some("builtin.turret.basic"),
        BlockKind::DronePort => Some("builtin.drone_port.basic"),
        _ => None,
    }
}

pub fn can_accept_item(kind: BlockKind, item: &ItemKind) -> bool {
    match kind {
        BlockKind::Wire | BlockKind::CpuNode => false,
        BlockKind::Turret => item == &ItemKind::Ammo,
        BlockKind::Conveyor | BlockKind::Router => true,
        BlockKind::Assembler => matches!(item, ItemKind::Ore | ItemKind::Plate),
        BlockKind::Core | BlockKind::Storage | BlockKind::DronePort => true,
        BlockKind::Drill => false,
    }
}

pub fn cpu_scaled_threshold(effective_cpu_rate: f32, base: u32) -> u32 {
    let speedup = (effective_cpu_rate / 8.0).clamp(0.1, 10.0);
    ((base as f32 / speedup).ceil() as u32).max(3)
}

fn inventory_capacity(kind: BlockKind) -> u32 {
    match kind {
        BlockKind::Core => 1000,
        BlockKind::Storage => 300,
        BlockKind::Conveyor | BlockKind::Router => 1,
        BlockKind::Turret => 80,
        BlockKind::Assembler => 100,
        BlockKind::Drill => 10,
        BlockKind::DronePort => 120,
        _ => 0,
    }
}

fn block_hp(kind: BlockKind) -> i32 {
    match kind {
        BlockKind::Wire => 15,
        BlockKind::Core => 500,
        _ => 90,
    }
}
