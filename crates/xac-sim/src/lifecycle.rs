use xac_core::{BlockKind, GameSnapshot, LogLevel};

use crate::wave;
use crate::Simulation;

impl Simulation {
    pub fn set_running(&mut self, running: bool) -> GameSnapshot {
        self.running = running && !self.core_defeated();
        self.snapshot()
    }

    pub fn step_ticks(&mut self, count: u32) -> GameSnapshot {
        for _ in 0..count.min(500) {
            self.tick_once();
        }
        self.snapshot()
    }

    pub fn update_if_running(&mut self, max_ticks: u32) -> GameSnapshot {
        if self.running {
            self.step_ticks(max_ticks)
        } else {
            self.snapshot()
        }
    }

    fn tick_once(&mut self) {
        if self.core_defeated() {
            self.running = false;
            return;
        }
        self.tick += 1;
        if wave::should_spawn_wave(self.tick) {
            self.spawn_wave(wave::current_wave(self.tick));
        }

        self.recompute_networks();
        self.run_programmable_behaviors();
        self.run_block_physics();
        self.run_drones();
        self.run_enemies();
        self.cleanup_destroyed();
    }

    fn cleanup_destroyed(&mut self) {
        let dead_enemies: Vec<_> = self
            .enemies
            .values()
            .filter(|e| e.hp <= 0)
            .map(|e| e.id.clone())
            .collect();
        for id in &dead_enemies {
            self.enemies.remove(id);
            self.log(LogLevel::Info, id.clone(), "enemy destroyed".to_string());
        }
        for block in self.blocks.values_mut() {
            if block
                .target_id
                .as_ref()
                .is_some_and(|target_id| dead_enemies.contains(target_id))
            {
                block.target_id = None;
            }
        }

        let destroyed_blocks: Vec<_> = self
            .blocks
            .values()
            .filter(|b| b.hp <= 0 && b.kind != BlockKind::Core)
            .map(|b| (b.id.clone(), b.kind, b.pos))
            .collect();
        for (id, kind, pos) in destroyed_blocks {
            self.blocks.remove(&id);
            self.set_tile_footprint(kind, pos, None);
            self.remove_block_references(&id);
            self.log(LogLevel::Warn, id, "block destroyed".to_string());
        }
        if let Some(core_id) = self
            .blocks
            .values()
            .find(|block| block.kind == BlockKind::Core && block.hp <= 0)
            .map(|block| block.id.clone())
        {
            let should_log = self
                .blocks
                .get(&core_id)
                .map(|block| block.status != "core breached")
                .unwrap_or(false);
            if let Some(core) = self.blocks.get_mut(&core_id) {
                core.hp = 0;
                core.status = "core breached".to_string();
            }
            self.running = false;
            if should_log {
                self.log(
                    LogLevel::Error,
                    core_id,
                    "core destroyed; simulation halted".to_string(),
                );
            }
        }
        self.recompute_networks();
    }

    pub(crate) fn core_hp(&self) -> i32 {
        self.blocks
            .values()
            .find(|block| block.kind == BlockKind::Core)
            .map(|block| block.hp.max(0))
            .unwrap_or(0)
    }

    fn core_defeated(&self) -> bool {
        self.core_hp() <= 0
    }
}
