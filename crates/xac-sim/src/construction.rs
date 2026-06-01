use anyhow::{anyhow, Result};
use xac_core::{BlockKind, Direction, EntityId, GameSnapshot, LogLevel, Pos};

use crate::block_defs::make_block;
use crate::geometry::footprint_positions;
use crate::Simulation;

impl Simulation {
    pub fn place_block(
        &mut self,
        kind: BlockKind,
        pos: Pos,
        dir: Direction,
    ) -> Result<GameSnapshot> {
        self.place_block_unchecked(kind, pos, dir)?;
        self.recompute_networks();
        Ok(self.snapshot())
    }

    pub fn place_blocks(
        &mut self,
        kind: BlockKind,
        positions: Vec<Pos>,
        dir: Direction,
    ) -> Result<GameSnapshot> {
        let mut placed = 0usize;
        let mut last_error = None;
        for pos in positions {
            match self.place_block_unchecked(kind, pos, dir) {
                Ok(()) => placed += 1,
                Err(error) => last_error = Some(error),
            }
        }
        self.recompute_networks();
        if placed == 0 {
            if let Some(error) = last_error {
                return Err(error);
            }
        }
        Ok(self.snapshot())
    }

    fn place_block_unchecked(&mut self, kind: BlockKind, pos: Pos, dir: Direction) -> Result<()> {
        if kind == BlockKind::Core {
            return Err(anyhow!(
                "core is the initial 4x4 objective and cannot be placed"
            ));
        }
        let footprint = footprint_positions(kind, pos);
        if footprint.iter().any(|tile| !self.in_bounds(*tile)) {
            return Err(anyhow!("position is outside the map"));
        }
        for tile in &footprint {
            let idx = self
                .tile_index(*tile)
                .ok_or_else(|| anyhow!("invalid tile"))?;
            if !self.tiles[idx].buildable || self.tiles[idx].block_id.is_some() {
                return Err(anyhow!("tile is not buildable or is already occupied"));
            }
        }

        let id = self.make_id(kind.as_str());
        let mut block = make_block(id.clone(), kind, pos, dir);
        block.behavior_ref = kind.default_behavior_id().map(ToOwned::to_owned);
        if kind == BlockKind::Turret {
            block.tags.push("frontline".to_string());
        }
        self.set_tile_footprint(kind, pos, Some(id.clone()));
        self.blocks.insert(id.clone(), block);
        self.fuel_banks.insert(id.clone(), 0.0);
        self.selected_id = Some(id.clone());
        self.log(
            LogLevel::Info,
            id,
            format!("placed {kind:?} at {},{}", pos.x, pos.y),
        );
        Ok(())
    }

    pub fn deconstruct_block(&mut self, block_id: &str) -> Result<GameSnapshot> {
        let Some(block) = self.blocks.get(block_id).cloned() else {
            return Err(anyhow!("unknown block: {block_id}"));
        };
        if block.kind == BlockKind::Core {
            return Err(anyhow!("core cannot be deconstructed"));
        }

        self.blocks.remove(block_id);
        self.set_tile_footprint(block.kind, block.pos, None);
        self.remove_block_references(block_id);
        if self.selected_id.as_deref() == Some(block_id) {
            self.selected_id = None;
        }
        self.log(
            LogLevel::Info,
            block_id.to_string(),
            format!("deconstructed {}", block.kind.as_str()),
        );
        self.recompute_networks();
        Ok(self.snapshot())
    }

    pub fn rotate_block(&mut self, block_id: &str) -> Result<GameSnapshot> {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return Err(anyhow!("unknown block: {block_id}"));
        };
        block.dir = block.dir.rotate_cw();
        let kind = block.kind;
        let dir = block.dir;
        block.status = format!("facing {dir:?}");
        self.log(
            LogLevel::Info,
            block_id.to_string(),
            format!("rotated {} to {dir:?}", kind.as_str()),
        );
        Ok(self.snapshot())
    }

    pub fn select_entity(&mut self, id: Option<EntityId>) -> GameSnapshot {
        self.selected_id = id;
        self.snapshot()
    }
}
