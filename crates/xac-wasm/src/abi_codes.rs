use xac_core::{Direction, EnemyKind, ItemKind};

pub(crate) fn direction_from_code(code: i32) -> Option<Direction> {
    match code {
        0 => Some(Direction::North),
        1 => Some(Direction::East),
        2 => Some(Direction::South),
        3 => Some(Direction::West),
        _ => None,
    }
}

pub(crate) fn direction_code(dir: Direction) -> i32 {
    match dir {
        Direction::North => 0,
        Direction::East => 1,
        Direction::South => 2,
        Direction::West => 3,
    }
}

pub(crate) fn direction_index(dir: Direction) -> usize {
    direction_code(dir) as usize
}

pub(crate) fn recipe_from_code(code: i32) -> Option<ItemKind> {
    match code {
        0 => Some(ItemKind::Plate),
        1 => Some(ItemKind::Ammo),
        _ => None,
    }
}

pub(crate) fn recipe_code(recipe: &ItemKind) -> i32 {
    match recipe {
        ItemKind::Plate => 0,
        ItemKind::Ammo => 1,
        _ => -1,
    }
}

pub(crate) fn recipe_index(recipe: &ItemKind) -> usize {
    match recipe {
        ItemKind::Plate => 0,
        ItemKind::Ammo => 1,
        _ => 0,
    }
}

pub(crate) fn item_from_code(code: i32) -> Option<ItemKind> {
    match code {
        0 => Some(ItemKind::Ore),
        1 => Some(ItemKind::Plate),
        2 => Some(ItemKind::Ammo),
        3 => Some(ItemKind::CpuPart),
        4 => Some(ItemKind::DronePart),
        _ => None,
    }
}

pub(crate) fn item_code(item: &ItemKind) -> i32 {
    match item {
        ItemKind::Ore => 0,
        ItemKind::Plate => 1,
        ItemKind::Ammo => 2,
        ItemKind::CpuPart => 3,
        ItemKind::DronePart => 4,
    }
}

pub(crate) fn enemy_kind_code(kind: &EnemyKind) -> i32 {
    match kind {
        EnemyKind::Grunt => 0,
        EnemyKind::Runner => 1,
        EnemyKind::Armored => 2,
        EnemyKind::WireCutter => 3,
    }
}

pub(crate) fn dropoff_tag_from_code(code: i32) -> Option<&'static str> {
    match code {
        0 => Some("frontline"),
        _ => None,
    }
}
