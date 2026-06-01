use xac_core::{BlockKind, Direction, ItemFlowEvent, ItemKind};

use crate::geometry::block_center;
use crate::Simulation;

const MAX_ITEM_FLOW_EVENTS: usize = 160;
const ITEM_FLOW_RETENTION_TICKS: u64 = 80;

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
        let Some(dst_id) = self.block_id_at(block.pos.step(block.dir)) else {
            return true;
        };
        !self
            .blocks
            .get(&dst_id)
            .map(|dst| dst.kind.can_accept_item(&ItemKind::Ore) && dst.inventory.has_space(1))
            .unwrap_or(false)
    }

    pub(crate) fn transfer_from(&mut self, block_id: &str, dir: Direction, amount: u32) -> bool {
        self.transfer_matching_item_from(block_id, dir, None, amount)
    }

    pub(crate) fn transfer_item_from(
        &mut self,
        block_id: &str,
        dir: Direction,
        item_kind: &ItemKind,
        amount: u32,
    ) -> bool {
        self.transfer_matching_item_from(block_id, dir, Some(item_kind), amount)
    }

    fn transfer_matching_item_from(
        &mut self,
        block_id: &str,
        dir: Direction,
        item_filter: Option<&ItemKind>,
        amount: u32,
    ) -> bool {
        let (kind, src_pos) = match self.blocks.get(block_id) {
            Some(block) => (block.kind, block.pos),
            None => return false,
        };
        let dst_pos = src_pos.step(dir);
        let Some(dst_id) = self.block_id_at(dst_pos) else {
            return false;
        };
        let Some((item_kind, available)) =
            self.transferable_item(block_id, &dst_id, item_filter, amount)
        else {
            return false;
        };
        let Some((from, to)) = self.flow_endpoints(block_id, &dst_id) else {
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
        self.record_item_flow(block_id, &dst_id, item_kind, moved, from, to);
        true
    }

    pub(crate) fn output_available(&self, block_id: &str, dir: Direction) -> bool {
        self.output_matching_item_available(block_id, None, dir)
    }

    pub(crate) fn output_item_available(
        &self,
        block_id: &str,
        item_kind: &ItemKind,
        dir: Direction,
    ) -> bool {
        self.output_matching_item_available(block_id, Some(item_kind), dir)
    }

    fn output_matching_item_available(
        &self,
        block_id: &str,
        item_filter: Option<&ItemKind>,
        dir: Direction,
    ) -> bool {
        let Some(src_pos) = self.blocks.get(block_id).map(|block| block.pos) else {
            return false;
        };
        let Some(dst_id) = self.block_id_at(src_pos.step(dir)) else {
            return false;
        };
        self.transferable_item(block_id, &dst_id, item_filter, 1)
            .is_some()
    }

    fn transferable_item(
        &self,
        src_id: &str,
        dst_id: &str,
        item_filter: Option<&ItemKind>,
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
            .find(|(item_kind, available)| {
                **available > 0
                    && item_filter.is_none_or(|filter| filter == *item_kind)
                    && dst.kind.can_accept_item(item_kind)
            })
            .map(|(kind, amount)| (kind.clone(), *amount))
    }

    fn flow_endpoints(
        &self,
        src_id: &str,
        dst_id: &str,
    ) -> Option<(xac_core::WorldPos, xac_core::WorldPos)> {
        Some((
            block_center(self.blocks.get(src_id)?),
            block_center(self.blocks.get(dst_id)?),
        ))
    }

    pub(crate) fn record_item_flow(
        &mut self,
        src_entity: &str,
        dst_entity: &str,
        item: ItemKind,
        amount: u32,
        from: xac_core::WorldPos,
        to: xac_core::WorldPos,
    ) {
        let id = format!("flow_{}", self.next_flow_id);
        self.next_flow_id += 1;
        self.item_flows.push_back(ItemFlowEvent {
            id,
            tick: self.tick,
            item,
            amount,
            from_entity: src_entity.to_string(),
            to_entity: dst_entity.to_string(),
            from,
            to,
        });
        while self.item_flows.len() > MAX_ITEM_FLOW_EVENTS {
            self.item_flows.pop_front();
        }
        while self
            .item_flows
            .front()
            .map(|event| self.tick.saturating_sub(event.tick) > ITEM_FLOW_RETENTION_TICKS)
            .unwrap_or(false)
        {
            self.item_flows.pop_front();
        }
    }
}
