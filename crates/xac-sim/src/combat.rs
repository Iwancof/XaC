use xac_core::{BlockKind, Enemy, EnemyKind, ItemKind, LogLevel, Pos, WorldPos};
use xac_wasm::TargetRule;

use crate::geometry::nearest_block_target;
use crate::wave;
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

    pub(crate) fn run_turret_scan_index(&mut self, turret_id: &str, index: u32) {
        let Some(turret) = self.blocks.get(turret_id).cloned() else {
            return;
        };
        if turret.kind != BlockKind::Turret || turret.inventory.count(&ItemKind::Ammo) == 0 {
            return;
        }
        let Some(enemy_id) = self
            .visible_turret_target_ids(turret.pos)
            .get(index as usize)
            .cloned()
        else {
            return;
        };
        if let Some(enemy) = self.enemies.get_mut(&enemy_id) {
            enemy.hp -= 12;
        }
        if let Some(block) = self.blocks.get_mut(turret_id) {
            block.inventory.remove(&ItemKind::Ammo, 1);
            block.status = format!("attacking scanned {index}: {enemy_id}");
        }
    }

    pub(crate) fn turret_visible_enemy_scan(&self, turret_id: &str) -> Vec<(EnemyKind, i32, f32)> {
        let Some(turret) = self.blocks.get(turret_id) else {
            return Vec::new();
        };
        if turret.kind != BlockKind::Turret {
            return Vec::new();
        }
        let origin = WorldPos::from_tile_center(turret.pos);
        let mut in_range: Vec<_> = self
            .enemies
            .values()
            .filter(|enemy| enemy.hp > 0 && origin.distance(enemy.pos) <= 8.0)
            .collect();
        in_range.sort_by(|a, b| origin.distance(a.pos).total_cmp(&origin.distance(b.pos)));
        in_range
            .into_iter()
            .map(|enemy| (enemy.kind, enemy.hp, origin.distance(enemy.pos)))
            .collect()
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
            if enemy.attack_cooldown > 0 {
                enemy.attack_cooldown -= 1;
            }
            if enemy.pos.distance(target_pos) <= 0.2 {
                if enemy.attack_cooldown == 0 {
                    if let Some(block) = self.blocks.get_mut(&target_id) {
                        block.hp -= enemy.kind.attack_damage();
                    }
                    enemy.attack_cooldown = enemy.kind.attack_cooldown_ticks();
                } else if let Some(block) = self.blocks.get_mut(&target_id) {
                    block.status = format!("under attack by {}", enemy.id);
                }
            } else {
                enemy.pos = enemy.pos.move_toward(target_pos, enemy.move_speed);
                if let Some(block) = self.blocks.get_mut(&target_id) {
                    block.status = format!("targeted by {}", enemy.id);
                }
            }
        }
    }

    pub(crate) fn spawn_wave(&mut self, wave_index: u32) {
        for (offset, kind) in wave::wave_enemies(wave_index).into_iter().enumerate() {
            self.spawn_enemy_in_lane(kind, offset as i32);
        }
        self.log(LogLevel::Warn, "wave", format!("wave {wave_index} contact"));
    }

    fn spawn_enemy_in_lane(&mut self, kind: EnemyKind, lane_offset: i32) {
        let id = self.make_id("enemy");
        let lane = ((self.tick as i32 / 40) + lane_offset) % 20;
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

    fn visible_turret_target_ids(&self, origin: Pos) -> Vec<String> {
        let origin = WorldPos::from_tile_center(origin);
        let mut in_range: Vec<_> = self
            .enemies
            .values()
            .filter(|enemy| enemy.hp > 0 && origin.distance(enemy.pos) <= 8.0)
            .collect();
        in_range.sort_by(|a, b| origin.distance(a.pos).total_cmp(&origin.distance(b.pos)));
        in_range.into_iter().map(|enemy| enemy.id.clone()).collect()
    }
}

pub(crate) fn enemy_at(id: String, kind: EnemyKind, pos: WorldPos) -> Enemy {
    let hp = kind.max_hp();
    Enemy {
        id,
        kind,
        pos,
        hp,
        max_hp: hp,
        move_speed: kind.move_speed(),
        attack_cooldown: 0,
        target_id: None,
    }
}
