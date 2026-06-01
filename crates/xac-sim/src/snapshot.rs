use xac_core::{BlockKind, EnemyKind, GameSnapshot, GameStatus};

use crate::wave;
use crate::Simulation;

impl Simulation {
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            tick: self.tick,
            running: self.running,
            width: self.width,
            height: self.height,
            tiles: self.tiles.clone(),
            blocks: self
                .blocks
                .values()
                .cloned()
                .map(|mut block| {
                    block.fuel_bank = self.fuel_banks.get(&block.id).copied().unwrap_or(0.0);
                    block
                })
                .collect(),
            enemies: self.enemies.values().cloned().collect(),
            drones: self.drones.values().cloned().collect(),
            networks: self.networks.values().cloned().collect(),
            logs: self.logs.iter().cloned().collect(),
            selected_id: self.selected_id.clone(),
            behaviors: self
                .behaviors
                .values()
                .map(|package| self.behavior_summary_with_usage(package))
                .collect(),
            pending_jobs: self.pending_jobs.clone(),
            item_flows: self.item_flows.iter().cloned().collect(),
            status: self.game_status(),
        }
    }

    fn game_status(&self) -> GameStatus {
        let core_hp = self.core_hp();
        GameStatus {
            wave: wave::current_wave(self.tick),
            next_wave_in: wave::next_wave_in(self.tick),
            core_hp,
            core_max_hp: BlockKind::Core.max_hp(),
            defeated: core_hp <= 0,
            wire_threats: self
                .enemies
                .values()
                .filter(|enemy| enemy.kind == EnemyKind::WireCutter && enemy.hp > 0)
                .count() as u32,
            damaged_wires: self
                .blocks
                .values()
                .filter(|block| {
                    block.kind == BlockKind::Wire && block.hp < BlockKind::Wire.max_hp()
                })
                .count() as u32,
            network_cpu: self.networks.values().map(|network| network.cpu_pool).sum(),
        }
    }
}
