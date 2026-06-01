use xac_core::{BlockKind, DeliveryJob, Drone, DroneState, ItemKind};
use xac_wasm::DronePortCommand;

use crate::geometry::block_center;
use crate::Simulation;

impl Simulation {
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

    pub(crate) fn claim_drone_job(&mut self, drone_id: &str) {
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
