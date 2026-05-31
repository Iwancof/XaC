use xac_core::{BlockKind, Direction, ItemKind, TerrainKind};

use crate::block_defs::{can_accept_item, cpu_scaled_threshold};
use crate::Simulation;

impl Simulation {
    pub(crate) fn run_block_physics(&mut self) {
        let ids: Vec<_> = self.blocks.keys().cloned().collect();

        for id in ids {
            let Some(kind) = self.blocks.get(&id).map(|b| b.kind) else {
                continue;
            };
            match kind {
                BlockKind::Drill | BlockKind::Conveyor | BlockKind::Assembler => {
                    let dir = self.blocks.get(&id).map(|b| b.dir).unwrap_or_default();
                    self.transfer_from(&id, dir, 1);
                }
                BlockKind::Router => {}
                _ => {}
            }
        }
    }

    pub(crate) fn run_drill(&mut self, block_id: &str) {
        let mine_ready = self
            .blocks
            .get(block_id)
            .map(|b| {
                self.tile_at(b.pos)
                    .is_some_and(|t| t.terrain == TerrainKind::OrePatch)
            })
            .unwrap_or(false);
        if !mine_ready {
            return;
        }
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.progress += 1;
            let threshold = cpu_scaled_threshold(block.effective_cpu_rate, 30);
            if block.progress >= threshold && block.inventory.has_space(1) {
                block.progress = 0;
                block.inventory.add(ItemKind::Ore, 1);
                block.status = "mined ore".to_string();
            }
        }
    }

    pub(crate) fn run_assembler(&mut self, block_id: &str) {
        let prefer_ammo = self
            .blocks
            .get(block_id)
            .map(|b| b.status.contains("ammo"))
            .unwrap_or(true);
        let Some(block) = self.blocks.get_mut(block_id) else {
            return;
        };
        block.progress += 1;
        let threshold = cpu_scaled_threshold(block.effective_cpu_rate, 40);
        if block.progress < threshold {
            return;
        }
        block.progress = 0;
        if prefer_ammo
            && block.inventory.count(&ItemKind::Plate) >= 1
            && block.inventory.has_space(2)
        {
            block.inventory.remove(&ItemKind::Plate, 1);
            block.inventory.add(ItemKind::Ammo, 2);
            block.status = "produced ammo".to_string();
        } else if block.inventory.count(&ItemKind::Ore) >= 2 && block.inventory.has_space(1) {
            block.inventory.remove(&ItemKind::Ore, 2);
            block.inventory.add(ItemKind::Plate, 1);
            block.status = "produced plate".to_string();
        }
    }

    pub(crate) fn output_blocked(&self, block_id: &str) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
            return true;
        };
        if !block.inventory.has_space(1) {
            return true;
        }
        let Some(dst_id) = self.block_id_at(block.pos.step(block.dir)) else {
            return false;
        };
        !self
            .blocks
            .get(&dst_id)
            .map(|dst| can_accept_item(dst.kind, &ItemKind::Ore) && dst.inventory.has_space(1))
            .unwrap_or(false)
    }

    pub(crate) fn can_produce(&self, block_id: &str) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
            return false;
        };
        if block.kind != BlockKind::Assembler {
            return false;
        }
        let can_make_ammo =
            block.inventory.count(&ItemKind::Plate) >= 1 && block.inventory.has_space(2);
        let can_make_plate =
            block.inventory.count(&ItemKind::Ore) >= 2 && block.inventory.has_space(1);
        can_make_ammo || can_make_plate
    }

    pub(crate) fn transfer_from(&mut self, block_id: &str, dir: Direction, amount: u32) -> bool {
        let (kind, src_pos) = match self.blocks.get(block_id) {
            Some(block) => (block.kind, block.pos),
            None => return false,
        };
        let dst_pos = src_pos.step(dir);
        let Some(dst_id) = self.block_id_at(dst_pos) else {
            return false;
        };
        let Some((item_kind, available)) = self.transferable_item(block_id, &dst_id, amount) else {
            return false;
        };
        let moved = {
            let Some(src) = self.blocks.get_mut(block_id) else {
                return false;
            };
            src.inventory.remove(&item_kind, amount.min(available))
        };
        if moved == 0 {
            return false;
        }
        if let Some(dst) = self.blocks.get_mut(&dst_id) {
            dst.inventory.add(item_kind.clone(), moved);
            dst.status = format!("received {}", item_kind.as_str());
        }
        if let Some(src) = self.blocks.get_mut(block_id) {
            src.status = match kind {
                BlockKind::Router => format!("routed {}", item_kind.as_str()),
                _ => format!("sent {}", item_kind.as_str()),
            };
        }
        true
    }

    fn transferable_item(
        &self,
        src_id: &str,
        dst_id: &str,
        amount: u32,
    ) -> Option<(ItemKind, u32)> {
        let src = self.blocks.get(src_id)?;
        let dst = self.blocks.get(dst_id)?;
        if !dst.inventory.has_space(amount) {
            return None;
        }
        src.inventory
            .items
            .iter()
            .find(|(item_kind, available)| **available > 0 && can_accept_item(dst.kind, item_kind))
            .map(|(kind, amount)| (kind.clone(), *amount))
    }
}
