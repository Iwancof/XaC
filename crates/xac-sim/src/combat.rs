use xac_core::{BlockKind, Enemy, EnemyKind, ItemKind, LogLevel, Pos, WorldPos};
use xac_wasm::TargetRule;

use crate::geometry::nearest_block_target;
use crate::wave;
use crate::Simulation;

const TURRET_DAMAGE: i32 = 12;

#[derive(Clone, Debug)]
struct VisibleTurretTarget {
    id: String,
    kind: EnemyKind,
    hp: i32,
    distance: f32,
}

impl Simulation {
    pub(crate) fn run_turret_once(&mut self, turret_id: &str, priority: &[TargetRule]) {
        let Some(turret) = self.blocks.get(turret_id).cloned() else {
            return;
        };
        if turret.kind != BlockKind::Turret {
            return;
        }
        self.clear_block_target(turret_id);
        if turret.inventory.count(&ItemKind::Ammo) == 0 {
            return;
        }
        if let Some(enemy_id) = self.choose_target(turret.pos, priority) {
            self.apply_turret_attack(turret_id, &enemy_id, format!("attacking {enemy_id}"));
        }
    }

    pub(crate) fn run_turret_scan_index(&mut self, turret_id: &str, index: u32) {
        let Some(turret) = self.blocks.get(turret_id).cloned() else {
            return;
        };
        if turret.kind != BlockKind::Turret {
            return;
        }
        self.clear_block_target(turret_id);
        if turret.inventory.count(&ItemKind::Ammo) == 0 {
            return;
        }
        let Some(target) = self
            .visible_turret_targets(turret.pos)
            .get(index as usize)
            .cloned()
        else {
            return;
        };
        self.apply_turret_attack(
            turret_id,
            &target.id,
            format!("attacking scanned {index}: {}", target.id),
        );
    }

    fn clear_block_target(&mut self, block_id: &str) {
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.target_id = None;
        }
    }

    pub(crate) fn turret_visible_enemy_scan(&self, turret_id: &str) -> Vec<(EnemyKind, i32, f32)> {
        let Some(turret) = self.blocks.get(turret_id) else {
            return Vec::new();
        };
        if turret.kind != BlockKind::Turret {
            return Vec::new();
        }
        self.visible_turret_targets(turret.pos)
            .into_iter()
            .map(|target| (target.kind, target.hp, target.distance))
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
        let targets = self.visible_turret_targets(origin);
        if targets.is_empty() {
            return None;
        }
        for rule in priority {
            match rule {
                TargetRule::Kind(kind) => {
                    if let Some(enemy) = targets
                        .iter()
                        .filter(|target| target.kind == *kind)
                        .min_by(|a, b| a.distance.total_cmp(&b.distance))
                    {
                        return Some(enemy.id.clone());
                    }
                }
                TargetRule::LowestHp => {
                    if let Some(enemy) = targets.iter().min_by_key(|target| target.hp) {
                        return Some(enemy.id.clone());
                    }
                }
                TargetRule::Nearest => {
                    if let Some(enemy) = targets
                        .iter()
                        .min_by(|a, b| a.distance.total_cmp(&b.distance))
                    {
                        return Some(enemy.id.clone());
                    }
                }
            }
        }
        None
    }

    fn visible_turret_targets(&self, origin: Pos) -> Vec<VisibleTurretTarget> {
        let origin = WorldPos::from_tile_center(origin);
        let range = BlockKind::Turret.attack_range_tiles().unwrap_or(0.0);
        let mut in_range: Vec<_> = self
            .enemies
            .values()
            .filter_map(|enemy| {
                let distance = origin.distance(enemy.pos);
                (enemy.hp > 0 && distance <= range).then_some(VisibleTurretTarget {
                    id: enemy.id.clone(),
                    kind: enemy.kind,
                    hp: enemy.hp,
                    distance,
                })
            })
            .collect();
        in_range.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        in_range
    }

    fn apply_turret_attack(&mut self, turret_id: &str, enemy_id: &str, status: String) {
        if let Some(enemy) = self.enemies.get_mut(enemy_id) {
            enemy.hp -= TURRET_DAMAGE;
        }
        if let Some(block) = self.blocks.get_mut(turret_id) {
            block.inventory.remove(&ItemKind::Ammo, 1);
            block.target_id = Some(enemy_id.to_string());
            block.status = status;
        }
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
