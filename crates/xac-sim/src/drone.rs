use xac_core::{
    BehaviorKind, BlockKind, DeliveryJob, Drone, DroneState, ItemKind, Pos, WorldPos,
    CARRIER_DRONE_BATTERY_CAPACITY, CARRIER_DRONE_BATTERY_RECHARGE_PER_TICK,
    CARRIER_DRONE_DOCKING_DISTANCE, CARRIER_DRONE_LOCAL_CPU_RATE,
    CARRIER_DRONE_LOGIC_FUEL_CAPACITY, CARRIER_DRONE_LOGIC_RECHARGE_PER_TICK,
    CARRIER_DRONE_MOVE_BATTERY_COST, CARRIER_DRONE_MOVE_SPEED, CARRIER_DRONE_WORK_BATTERY_COST,
};
use xac_wasm::{BehaviorHostInput, BehaviorIntent, DroneCommand, DronePortCommand};

use crate::cpu::FuelPolicy;
use crate::geometry::{block_center, closest_point_on_block};
use crate::Simulation;

const DRONE_BEHAVIOR_FUEL_POLICY: FuelPolicy = FuelPolicy {
    min_invocation_fuel: 40,
    max_bank_seconds: 8.0,
};

impl Simulation {
    pub(crate) fn run_drones(&mut self) {
        let drone_ids: Vec<_> = self.drones.keys().cloned().collect();
        for drone_id in drone_ids {
            if let Some(command) = self.run_carrier_drone_behavior(&drone_id) {
                self.apply_drone_command(&drone_id, command);
            } else {
                self.continue_drone_activity(&drone_id);
            }
        }
    }

    pub(crate) fn apply_drone_port_commands(
        &mut self,
        port_id: &str,
        commands: Vec<DronePortCommand>,
    ) {
        for command in commands {
            match command {
                DronePortCommand::AutoDispatch => self.ensure_drone_and_job(port_id),
                DronePortCommand::ChargeDockedDrones => {
                    self.charge_docked_drones_at_port(port_id);
                }
                DronePortCommand::CreateDeliveryJob {
                    item,
                    amount,
                    dropoff_tag,
                } => {
                    self.create_delivery_job_from_port(port_id, item, amount, &dropoff_tag, 50);
                }
                DronePortCommand::DispatchIdleDrones => {
                    self.dispatch_idle_drones_from_port(port_id);
                }
            }
        }
    }

    pub(crate) fn ensure_drone_and_job(&mut self, port_id: &str) {
        self.ensure_carrier_drone(port_id);
        if !self.tick.is_multiple_of(60) {
            return;
        }
        self.create_delivery_job_from_port(port_id, ItemKind::Ammo, 10, "frontline", 50);
    }

    pub(crate) fn docked_drone_count_at_port(&self, port_id: &str) -> i32 {
        let count = self
            .drones
            .values()
            .filter(|drone| drone.home_port == port_id && self.drone_at_home(&drone.id))
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    fn ensure_carrier_drone(&mut self, port_id: &str) {
        let Some(port) = self.blocks.get(port_id).cloned() else {
            return;
        };
        if self.drones.values().any(|drone| drone.home_port == port_id) {
            return;
        }
        let id = self.make_id("drone");
        self.drones.insert(
            id.clone(),
            Drone::carrier(
                id,
                port_id.to_string(),
                Some("builtin.carrier_drone.basic".to_string()),
                block_center(&port),
            ),
        );
    }

    fn charge_docked_drones_at_port(&mut self, port_id: &str) {
        let drone_ids: Vec<_> = self
            .drones
            .values()
            .filter(|drone| drone.home_port == port_id)
            .map(|drone| drone.id.clone())
            .collect();
        for drone_id in drone_ids {
            if self.drone_at_home(&drone_id) {
                self.charge_docked_drone(&drone_id);
            }
        }
    }

    fn dispatch_idle_drones_from_port(&mut self, port_id: &str) {
        self.ensure_carrier_drone(port_id);
        let drone_ids: Vec<_> = self
            .drones
            .values()
            .filter(|drone| drone.home_port == port_id && drone.job.is_none())
            .map(|drone| drone.id.clone())
            .collect();
        for drone_id in drone_ids {
            self.claim_drone_job(&drone_id);
        }
    }

    fn create_delivery_job_from_port(
        &mut self,
        port_id: &str,
        item: ItemKind,
        amount: u32,
        dropoff_tag: &str,
        priority: i32,
    ) -> bool {
        if amount == 0 {
            return false;
        }
        let Some(dropoff) = self.find_delivery_dropoff(&item, amount, dropoff_tag) else {
            return false;
        };
        if self.delivery_job_exists(&dropoff, &item) {
            return false;
        }
        let Some(pickup) = self.find_delivery_pickup(port_id, &item, amount) else {
            return false;
        };
        let job_id = self.make_id("job");
        self.pending_jobs.push(DeliveryJob {
            id: job_id,
            item: item.clone(),
            amount,
            pickup,
            dropoff,
            priority,
        });
        if let Some(port) = self.blocks.get_mut(port_id) {
            port.status = format!("queued {} delivery", item.as_str());
        }
        true
    }

    fn find_delivery_dropoff(
        &self,
        item: &ItemKind,
        amount: u32,
        dropoff_tag: &str,
    ) -> Option<String> {
        self.blocks
            .values()
            .find(|block| {
                block.tags.iter().any(|tag| tag == dropoff_tag)
                    && block.kind.can_accept_item(item)
                    && block.inventory.count(item) < amount
                    && block.inventory.has_space(amount)
            })
            .map(|block| block.id.clone())
    }

    fn find_delivery_pickup(&self, port_id: &str, item: &ItemKind, amount: u32) -> Option<String> {
        let candidates: Vec<String> = self
            .blocks
            .get(port_id)
            .and_then(|port| port.network_id)
            .and_then(|network_id| self.networks.get(&network_id))
            .map(|network| network.block_ids.clone())
            .unwrap_or_else(|| vec![port_id.to_string()]);
        candidates.into_iter().find(|id| {
            self.blocks
                .get(id)
                .map(|block| {
                    matches!(
                        block.kind,
                        BlockKind::Storage
                            | BlockKind::Core
                            | BlockKind::Assembler
                            | BlockKind::DronePort
                    ) && block.inventory.count(item) >= amount
                })
                .unwrap_or(false)
        })
    }

    fn run_carrier_drone_behavior(&mut self, drone_id: &str) -> Option<DroneCommand> {
        let (behavior_ref, battery, logic_fuel, has_job, cargo_counts, cargo_free) = {
            let drone = self.drones.get(drone_id)?;
            let cargo_free = drone.cargo.capacity.saturating_sub(drone.cargo.total());
            let cargo_counts = drone
                .cargo
                .items
                .iter()
                .map(|(item, amount)| (item.clone(), i32::try_from(*amount).unwrap_or(i32::MAX)))
                .collect();
            (
                drone.behavior_ref.clone()?,
                drone.battery,
                drone.logic_fuel,
                drone.job.is_some(),
                cargo_counts,
                i32::try_from(cargo_free).unwrap_or(i32::MAX),
            )
        };
        if logic_fuel == 0 {
            return None;
        }

        let at_home = self.drone_at_home(drone_id);
        let (contact_inventory_counts, contact_space_counts) =
            self.drone_physical_host_counts(drone_id);
        let cpu_rate = self.carrier_drone_cpu_rate(drone_id, at_home);
        let available_fuel = self.grant_fuel_bank(drone_id, cpu_rate, DRONE_BEHAVIOR_FUEL_POLICY);
        if available_fuel == 0 {
            return None;
        }
        let fuel = available_fuel.min(logic_fuel);
        let compiled = match self.compiled_behavior(&behavior_ref, BehaviorKind::CarrierDrone) {
            Ok(compiled) => compiled,
            Err(error) => {
                self.log(
                    xac_core::LogLevel::Error,
                    drone_id.to_string(),
                    error.to_string(),
                );
                return None;
            }
        };
        let input = BehaviorHostInput {
            drone_battery_percent: battery.round().clamp(0.0, 100.0) as i32,
            drone_logic_fuel: logic_fuel,
            drone_has_job: has_job,
            drone_has_pending_job: !self.pending_jobs.is_empty(),
            drone_can_move: battery >= CARRIER_DRONE_MOVE_BATTERY_COST,
            drone_can_return_to_port: at_home || battery >= CARRIER_DRONE_MOVE_BATTERY_COST,
            drone_can_work: battery >= CARRIER_DRONE_WORK_BATTERY_COST,
            drone_can_idle: at_home || battery >= CARRIER_DRONE_MOVE_BATTERY_COST,
            drone_cargo_free: cargo_free,
            drone_cargo_counts: cargo_counts,
            drone_contact_inventory_counts: contact_inventory_counts,
            drone_contact_space_counts: contact_space_counts,
            ..Default::default()
        };
        let eval = match self.runtime.evaluate_compiled(&compiled, fuel, input) {
            Ok(eval) => eval,
            Err(error) => {
                self.spend_fuel_bank(drone_id, fuel);
                self.spend_drone_logic_fuel(drone_id, fuel);
                self.log(
                    xac_core::LogLevel::Error,
                    drone_id.to_string(),
                    error.to_string(),
                );
                return None;
            }
        };
        self.record_drone_behavior_runtime(drone_id, fuel, &eval);
        self.spend_fuel_bank(drone_id, eval.fuel_spent);
        self.spend_drone_logic_fuel(drone_id, eval.fuel_spent);
        if eval.over_budget {
            self.log(
                xac_core::LogLevel::Warn,
                drone_id.to_string(),
                format!("drone over_budget with {fuel} fuel"),
            );
            return None;
        }
        self.apply_behavior_logs(drone_id, eval.logs);
        match eval.intent {
            BehaviorIntent::CarrierDrone { command } => Some(command),
            _ => None,
        }
    }

    fn drone_physical_host_counts(
        &self,
        drone_id: &str,
    ) -> (
        std::collections::BTreeMap<ItemKind, i32>,
        std::collections::BTreeMap<ItemKind, i32>,
    ) {
        let Some(block_id) = self.drone_contact_block_id(drone_id) else {
            return Default::default();
        };
        let Some(block) = self.blocks.get(&block_id) else {
            return Default::default();
        };
        let inventory_counts = block
            .inventory
            .items
            .iter()
            .map(|(item, amount)| (item.clone(), i32::try_from(*amount).unwrap_or(i32::MAX)))
            .collect();
        let free = block
            .inventory
            .capacity
            .saturating_sub(block.inventory.total());
        let free = i32::try_from(free).unwrap_or(i32::MAX);
        let space_counts = ItemKind::all()
            .into_iter()
            .filter(|item| block.kind.can_accept_item(item))
            .map(|item| (item, free))
            .collect();
        (inventory_counts, space_counts)
    }

    fn carrier_drone_cpu_rate(&self, drone_id: &str, at_home: bool) -> f32 {
        let mut rate = CARRIER_DRONE_LOCAL_CPU_RATE;
        if !at_home {
            return rate;
        }
        let Some(network_id) = self
            .drones
            .get(drone_id)
            .and_then(|drone| self.blocks.get(&drone.home_port))
            .and_then(|port| port.network_id)
        else {
            return rate;
        };
        rate += self
            .networks
            .get(&network_id)
            .map(|network| network.effective_per_device)
            .unwrap_or(0.0);
        rate
    }

    fn spend_drone_logic_fuel(&mut self, drone_id: &str, fuel_spent: u64) {
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.logic_fuel = drone.logic_fuel.saturating_sub(fuel_spent);
        }
    }

    fn apply_drone_command(&mut self, drone_id: &str, command: DroneCommand) {
        match command {
            DroneCommand::ReturnToPort => self.return_drone_to_home(drone_id),
            DroneCommand::ClaimDeliveryJob => self.claim_drone_job(drone_id),
            DroneCommand::Deliver => {
                if let Some(job) = self.drones.get(drone_id).and_then(|d| d.job.clone()) {
                    self.run_drone_job(drone_id, job);
                } else {
                    self.idle_drone(drone_id);
                }
            }
            DroneCommand::MoveTo { pos } => self.move_drone_to_tile(drone_id, pos),
            DroneCommand::Load { item, amount } => self.load_drone_cargo(drone_id, item, amount),
            DroneCommand::Unload { item, amount } => {
                self.unload_drone_cargo(drone_id, item, amount);
            }
            DroneCommand::Idle => self.idle_drone(drone_id),
        }
    }

    fn continue_drone_activity(&mut self, drone_id: &str) {
        if let Some(job) = self
            .drones
            .get(drone_id)
            .and_then(|drone| drone.job.clone())
        {
            self.run_drone_job(drone_id, job);
            return;
        }
        if self
            .drones
            .get(drone_id)
            .map(|drone| drone.state == DroneState::Returning)
            .unwrap_or(false)
        {
            self.return_drone_to_home(drone_id);
            return;
        }
        if self.drone_at_home(drone_id) {
            self.charge_docked_drone(drone_id);
        }
    }

    fn move_drone_to_tile(&mut self, drone_id: &str, pos: Pos) {
        if !self.ensure_drone_battery(drone_id, CARRIER_DRONE_MOVE_BATTERY_COST) {
            return;
        }
        let target = WorldPos::from_tile_center(pos);
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.state = DroneState::Offline;
            drone.pos = drone.pos.move_toward(target, CARRIER_DRONE_MOVE_SPEED);
            drone.battery = (drone.battery - CARRIER_DRONE_MOVE_BATTERY_COST).max(0.0);
        }
        if self.drone_at_home(drone_id) {
            self.charge_docked_drone(drone_id);
        }
    }

    fn load_drone_cargo(&mut self, drone_id: &str, item: ItemKind, amount: u32) {
        let Some(block_id) = self.drone_contact_block_id(drone_id) else {
            return;
        };
        let free = self
            .drones
            .get(drone_id)
            .map(|drone| drone.cargo.capacity.saturating_sub(drone.cargo.total()))
            .unwrap_or(0);
        let requested = amount.min(free);
        if requested == 0 {
            return;
        }
        if !self.ensure_drone_battery(drone_id, CARRIER_DRONE_WORK_BATTERY_COST) {
            return;
        }
        let loaded = self
            .blocks
            .get_mut(&block_id)
            .map(|block| block.inventory.remove(&item, requested))
            .unwrap_or(0);
        if loaded == 0 {
            return;
        }
        let at_home = self.drone_at_home(drone_id);
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.cargo.add(item.clone(), loaded);
            drone.battery = (drone.battery - CARRIER_DRONE_WORK_BATTERY_COST).max(0.0);
            drone.state = if at_home {
                DroneState::Docked
            } else {
                DroneState::Offline
            };
        }
        if let Some(block) = self.blocks.get_mut(&block_id) {
            block.status = format!("drone loaded {}", item.as_str());
        }
    }

    fn unload_drone_cargo(&mut self, drone_id: &str, item: ItemKind, amount: u32) {
        let Some(block_id) = self.drone_contact_block_id(drone_id) else {
            return;
        };
        let Some(block) = self.blocks.get(&block_id) else {
            return;
        };
        if !block.kind.can_accept_item(&item) {
            return;
        }
        let free = block
            .inventory
            .capacity
            .saturating_sub(block.inventory.total());
        let requested = amount.min(free);
        if requested == 0 {
            return;
        }
        if !self.ensure_drone_battery(drone_id, CARRIER_DRONE_WORK_BATTERY_COST) {
            return;
        }
        let unloaded = self
            .drones
            .get_mut(drone_id)
            .map(|drone| {
                let unloaded = drone.cargo.remove(&item, requested);
                if unloaded > 0 {
                    drone.battery = (drone.battery - CARRIER_DRONE_WORK_BATTERY_COST).max(0.0);
                }
                unloaded
            })
            .unwrap_or(0);
        if unloaded == 0 {
            return;
        }
        if let Some(block) = self.blocks.get_mut(&block_id) {
            block.inventory.add(item.clone(), unloaded);
            block.status = format!("drone unloaded {}", item.as_str());
        }
        let at_home = self.drone_at_home(drone_id);
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.state = if at_home {
                DroneState::Docked
            } else {
                DroneState::Offline
            };
        }
    }

    fn drone_contact_block_id(&self, drone_id: &str) -> Option<String> {
        let pos = self.drones.get(drone_id)?.pos;
        self.blocks
            .values()
            .filter_map(|block| {
                let distance = pos.distance(closest_point_on_block(pos, block));
                (distance <= CARRIER_DRONE_DOCKING_DISTANCE).then_some((distance, block.id.clone()))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, id)| id)
    }

    fn idle_drone(&mut self, drone_id: &str) {
        if self.drone_at_home(drone_id) {
            self.charge_docked_drone(drone_id);
        } else {
            self.return_drone_to_home(drone_id);
        }
    }

    fn ensure_drone_battery(&mut self, drone_id: &str, cost: f32) -> bool {
        if self
            .drones
            .get(drone_id)
            .map(|drone| drone.battery >= cost)
            .unwrap_or(false)
        {
            return true;
        }
        if self.drone_at_home(drone_id) {
            self.charge_docked_drone(drone_id);
        } else if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.state = DroneState::Offline;
        }
        false
    }

    fn claim_drone_job(&mut self, drone_id: &str) {
        if self
            .drones
            .get(drone_id)
            .and_then(|drone| drone.job.as_ref())
            .is_some()
        {
            return;
        }
        if !self.drone_at_home(drone_id) {
            self.return_drone_to_home(drone_id);
            return;
        }

        self.charge_docked_drone(drone_id);
        if let Some(job) = self.take_next_job() {
            if let Some(drone) = self.drones.get_mut(drone_id) {
                drone.job = Some(job);
                drone.state = DroneState::Delivering;
            }
        }
    }

    fn charge_docked_drone(&mut self, drone_id: &str) {
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.state = DroneState::Docked;
            drone.battery = (drone.battery + CARRIER_DRONE_BATTERY_RECHARGE_PER_TICK)
                .min(CARRIER_DRONE_BATTERY_CAPACITY);
            drone.logic_fuel = (drone.logic_fuel + CARRIER_DRONE_LOGIC_RECHARGE_PER_TICK)
                .min(CARRIER_DRONE_LOGIC_FUEL_CAPACITY);
        }
    }

    fn return_drone_to_home(&mut self, drone_id: &str) {
        let Some(home_pos) = self.drone_home_pos(drone_id) else {
            return;
        };
        if self.drone_at_home(drone_id) {
            self.charge_docked_drone(drone_id);
            return;
        }
        if !self.ensure_drone_battery(drone_id, CARRIER_DRONE_MOVE_BATTERY_COST) {
            return;
        }
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.state = DroneState::Returning;
            drone.pos = drone.pos.move_toward(home_pos, CARRIER_DRONE_MOVE_SPEED);
            drone.battery = (drone.battery - CARRIER_DRONE_MOVE_BATTERY_COST).max(0.0);
        }
    }

    fn drone_at_home(&self, drone_id: &str) -> bool {
        let Some(home_pos) = self.drone_home_pos(drone_id) else {
            return false;
        };
        self.drones
            .get(drone_id)
            .map(|drone| drone.pos.distance(home_pos) <= CARRIER_DRONE_DOCKING_DISTANCE)
            .unwrap_or(false)
    }

    fn drone_home_pos(&self, drone_id: &str) -> Option<WorldPos> {
        self.drones
            .get(drone_id)
            .and_then(|drone| self.blocks.get(&drone.home_port))
            .map(block_center)
    }

    fn run_drone_job(&mut self, drone_id: &str, job: DeliveryJob) {
        let pickup_pos = self.blocks.get(&job.pickup).map(block_center);
        let dropoff_pos = self.blocks.get(&job.dropoff).map(block_center);
        let Some(dropoff_pos) = dropoff_pos else {
            self.clear_drone_job(drone_id);
            return;
        };

        let mut completed = false;
        if !self.ensure_drone_battery(drone_id, CARRIER_DRONE_WORK_BATTERY_COST) {
            return;
        }
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.battery = (drone.battery - CARRIER_DRONE_WORK_BATTERY_COST).max(0.0);
            drone.logic_fuel = drone.logic_fuel.saturating_sub(1);
            if drone.cargo.count(&job.item) == 0 {
                if let Some(pickup_pos) = pickup_pos {
                    if drone.pos.distance(pickup_pos) <= CARRIER_DRONE_DOCKING_DISTANCE {
                        let loaded = self
                            .blocks
                            .get_mut(&job.pickup)
                            .map(|b| b.inventory.remove(&job.item, job.amount))
                            .unwrap_or(0);
                        if loaded == 0 {
                            completed = true;
                        } else {
                            drone.cargo.add(job.item.clone(), loaded);
                        }
                    } else {
                        drone.pos = drone.pos.move_toward(pickup_pos, CARRIER_DRONE_MOVE_SPEED);
                    }
                } else {
                    completed = true;
                }
            } else if drone.pos.distance(dropoff_pos) <= CARRIER_DRONE_DOCKING_DISTANCE {
                let delivered = drone.cargo.remove(&job.item, job.amount);
                if let Some(block) = self.blocks.get_mut(&job.dropoff) {
                    block.inventory.add(job.item.clone(), delivered);
                    block.status = format!("drone delivered {}", job.item.as_str());
                }
                completed = true;
            } else {
                drone.pos = drone.pos.move_toward(dropoff_pos, CARRIER_DRONE_MOVE_SPEED);
            }
        }

        if completed {
            self.clear_drone_job(drone_id);
        }
    }

    fn clear_drone_job(&mut self, drone_id: &str) {
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.job = None;
            drone.state = DroneState::Returning;
        }
    }

    fn take_next_job(&mut self) -> Option<DeliveryJob> {
        let (index, _) = self
            .pending_jobs
            .iter()
            .enumerate()
            .max_by_key(|(_, job)| job.priority)?;
        Some(self.pending_jobs.remove(index))
    }

    fn delivery_job_exists(&self, dropoff: &str, item: &ItemKind) -> bool {
        self.pending_jobs
            .iter()
            .any(|job| job.dropoff == dropoff && &job.item == item)
            || self.drones.values().any(|drone| {
                drone
                    .job
                    .as_ref()
                    .map(|job| job.dropoff == dropoff && &job.item == item)
                    .unwrap_or(false)
            })
    }
}
