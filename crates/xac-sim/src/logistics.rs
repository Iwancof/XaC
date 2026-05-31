use xac_core::{BlockKind, Direction, ItemKind};

use crate::block_defs::can_accept_item;
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

    pub(crate) fn output_available(&self, block_id: &str, dir: Direction) -> bool {
        let Some(src_pos) = self.blocks.get(block_id).map(|block| block.pos) else {
            return false;
        };
        let Some(dst_id) = self.block_id_at(src_pos.step(dir)) else {
            return false;
        };
        self.transferable_item(block_id, &dst_id, 1).is_some()
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
