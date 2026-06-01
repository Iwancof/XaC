use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use xac_core::{
    Block, DeliveryJob, Drone, Enemy, EntityId, ItemFlowEvent, LogEntry, LogLevel, Network, Tile,
};

use crate::behavior::load_behaviors;
use crate::Simulation;

const SAVE_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct SaveFile {
    version: u32,
    tick: u64,
    running: bool,
    width: i32,
    height: i32,
    tiles: Vec<Tile>,
    blocks: BTreeMap<EntityId, Block>,
    enemies: BTreeMap<EntityId, Enemy>,
    drones: BTreeMap<EntityId, Drone>,
    networks: BTreeMap<u32, Network>,
    fuel_banks: BTreeMap<EntityId, f32>,
    pending_jobs: Vec<DeliveryJob>,
    item_flows: Vec<ItemFlowEvent>,
    logs: Vec<LogEntry>,
    selected_id: Option<EntityId>,
    next_id: u64,
    next_flow_id: u64,
}

impl Simulation {
    pub fn save_world(&mut self, slot: &str) -> Result<xac_core::GameSnapshot> {
        let path = save_path(&self.config_root, slot)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create save directory {}", parent.display()))?;
        }
        self.log(
            LogLevel::Info,
            "system",
            format!("world saved to {}", save_name(slot)?),
        );
        let save_file = SaveFile::from_simulation(self);
        let source = serde_json::to_string_pretty(&save_file).context("serialize world save")?;
        fs::write(&path, source).with_context(|| format!("write world save {}", path.display()))?;
        Ok(self.snapshot())
    }

    pub fn load_world(&mut self, slot: &str) -> Result<xac_core::GameSnapshot> {
        let path = save_path(&self.config_root, slot)?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read world save {}", path.display()))?;
        let save_file: SaveFile = serde_json::from_str(&source)
            .with_context(|| format!("parse world save {}", path.display()))?;
        save_file.apply_to(self)?;
        self.log(
            LogLevel::Info,
            "system",
            format!("world loaded from {}", save_name(slot)?),
        );
        Ok(self.snapshot())
    }
}

impl SaveFile {
    fn from_simulation(sim: &Simulation) -> Self {
        Self {
            version: SAVE_VERSION,
            tick: sim.tick,
            running: sim.running,
            width: sim.width,
            height: sim.height,
            tiles: sim.tiles.clone(),
            blocks: sim.blocks.clone(),
            enemies: sim.enemies.clone(),
            drones: sim.drones.clone(),
            networks: sim.networks.clone(),
            fuel_banks: sim.fuel_banks.clone(),
            pending_jobs: sim.pending_jobs.clone(),
            item_flows: sim.item_flows.iter().cloned().collect(),
            logs: sim.logs.iter().cloned().collect(),
            selected_id: sim.selected_id.clone(),
            next_id: sim.next_id,
            next_flow_id: sim.next_flow_id,
        }
    }

    fn apply_to(self, sim: &mut Simulation) -> Result<()> {
        if self.version != SAVE_VERSION {
            return Err(anyhow!(
                "unsupported save version {} (expected {})",
                self.version,
                SAVE_VERSION
            ));
        }

        sim.tick = self.tick;
        sim.running = self.running;
        sim.width = self.width;
        sim.height = self.height;
        sim.tiles = self.tiles;
        sim.blocks = self.blocks;
        sim.enemies = self.enemies;
        sim.drones = self.drones;
        sim.networks = self.networks;
        sim.fuel_banks = self.fuel_banks;
        sim.pending_jobs = self.pending_jobs;
        sim.item_flows = VecDeque::from(self.item_flows);
        sim.logs = VecDeque::from(self.logs);
        sim.selected_id = self.selected_id;
        sim.next_id = self.next_id;
        sim.next_flow_id = self.next_flow_id;
        sim.behaviors = load_behaviors(&sim.config_root)?;
        sim.compiled_behaviors.clear();
        sim.reserve_next_id_from_existing();
        sim.next_flow_id = sim.next_flow_id.max(max_flow_id(sim) + 1);
        sim.recompute_networks();
        Ok(())
    }
}

fn save_path(config_root: &std::path::Path, slot: &str) -> Result<PathBuf> {
    Ok(config_root
        .join("projects/default_project/saves")
        .join(format!("{}.json", save_name(slot)?)))
}

fn save_name(slot: &str) -> Result<String> {
    let trimmed = slot.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("save slot cannot be empty"));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(anyhow!(
            "save slot can only contain letters, numbers, '-' and '_'"
        ));
    }
    Ok(trimmed.to_string())
}

fn max_flow_id(sim: &Simulation) -> u64 {
    sim.item_flows
        .iter()
        .filter_map(|flow| flow.id.rsplit_once('_'))
        .filter_map(|(_, suffix)| suffix.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
}
