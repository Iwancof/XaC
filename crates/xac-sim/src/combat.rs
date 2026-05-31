use xac_core::{BlockKind, Enemy, EnemyKind, ItemKind, LogLevel, Pos, WorldPos};
use xac_wasm::TargetRule;

use crate::geometry::nearest_block_target;
use crate::Simulation;

impl Simulation {
    pub(crate) fn run_turret_once(&mut self, turret_id: &str, priority: &[TargetRule]) {
        let Some(turret) = self.blocks.get(turret_id).cloned() else {
            return;
        };
        if turret.kind != BlockKind::Turret || turret.inventory.count(&ItemKind::Ammo) == 0 {
            return;
        }
        let target = self.choose_target(turret.pos, priority);
        if let Some(enemy_id) = target {
            if let Some(enemy) = self.enemies.get_mut(&enemy_id) {
                enemy.hp -= 12;
            }
            if let Some(block) = self.blocks.get_mut(turret_id) {
                block.inventory.remove(&ItemKind::Ammo, 1);
                block.status = format!("attacking {enemy_id}");
            }
        }
    }

    pub(crate) fn run_enemies(&mut self) {
        let enemy_ids: Vec<_> = self.enemies.keys().cloned().collect();
        let blocks_snapshot = self.blocks.clone();

        for enemy_id in enemy_ids {
            let Some(enemy) = self.enemies.get_mut(&enemy_id) else {
                continue;
            };
            let target = if enemy.kind == EnemyKind::WireCutter {
                nearest_block_target(&blocks_snapshot, enemy.pos, |kind| {
                    matches!(
                        kind,
                        BlockKind::Wire | BlockKind::CpuNode | BlockKind::DronePort
                    )
                })
                .or_else(|| {
                    nearest_block_target(&blocks_snapshot, enemy.pos, |kind| {
                        kind == BlockKind::Core
                    })
                })
            } else {
                nearest_block_target(&blocks_snapshot, enemy.pos, |kind| kind == BlockKind::Core)
            };
            let Some((target_id, target_pos)) = target else {
                continue;
            };

            enemy.target_id = Some(target_id.clone());
            if enemy.pos.distance(target_pos) <= 0.2 {
                if let Some(block) = self.blocks.get_mut(&target_id) {
                    block.hp -= if enemy.kind == EnemyKind::Armored {
                        8
                    } else {
                        5
                    };
                }
            } else {
                enemy.pos = enemy.pos.move_toward(target_pos, enemy.move_speed);
            }
        }
    }

    pub(crate) fn spawn_wave_enemy(&mut self) {
        let kind = match (self.tick / 80) % 4 {
            0 => EnemyKind::Grunt,
            1 => EnemyKind::Runner,
            2 => EnemyKind::Armored,
            _ => EnemyKind::Grunt,
        };
        self.spawn_enemy(kind);
    }

    pub(crate) fn spawn_enemy(&mut self, kind: EnemyKind) {
        let id = self.make_id("enemy");
        let lane = (self.tick as i32 / 40) % 20;
        let pos = WorldPos {
            x: 4.5 + lane as f32,
            y: 4.5,
        };
        self.enemies
            .insert(id.clone(), enemy_at(id.clone(), kind, pos));
        self.log(LogLevel::Warn, id, format!("{kind:?} wave contact"));
    }

    fn choose_target(&self, origin: Pos, priority: &[TargetRule]) -> Option<String> {
        let origin = WorldPos::from_tile_center(origin);
        let in_range: Vec<_> = self
            .enemies
            .values()
            .filter(|e| e.hp > 0 && origin.distance(e.pos) <= 8.0)
            .collect();
        if in_range.is_empty() {
            return None;
        }
        for rule in priority {
            match rule {
                TargetRule::Kind(kind) => {
                    if let Some(enemy) = in_range
                        .iter()
                        .filter(|e| e.kind == *kind)
                        .min_by(|a, b| origin.distance(a.pos).total_cmp(&origin.distance(b.pos)))
                    {
                        return Some(enemy.id.clone());
                    }
                }
                TargetRule::LowestHp => {
                    if let Some(enemy) = in_range.iter().min_by_key(|e| e.hp) {
                        return Some(enemy.id.clone());
                    }
                }
                TargetRule::Nearest => {
                    if let Some(enemy) = in_range
                        .iter()
                        .min_by(|a, b| origin.distance(a.pos).total_cmp(&origin.distance(b.pos)))
                    {
                        return Some(enemy.id.clone());
                    }
                }
            }
        }
        None
    }
}

pub(crate) fn enemy_at(id: String, kind: EnemyKind, pos: WorldPos) -> Enemy {
    let (hp, speed_ticks, move_speed) = match kind {
        EnemyKind::Grunt => (30, 8, 0.07),
        EnemyKind::Runner => (20, 3, 0.14),
        EnemyKind::Armored => (90, 12, 0.045),
        EnemyKind::WireCutter => (38, 5, 0.10),
    };
    Enemy {
        id,
        kind,
        pos,
        hp,
        max_hp: hp,
        speed_ticks,
        move_cooldown: 0,
        move_speed,
        target_id: None,
    }
}
