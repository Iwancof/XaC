use xac_core::{BlockKind, DeliveryJob, Drone, DroneState, Inventory, ItemKind};

use crate::geometry::block_center;
use crate::Simulation;

const DRONE_MOVE_SPEED: f32 = 0.18;
const DOCKING_DISTANCE: f32 = 0.15;

impl Simulation {
    pub(crate) fn run_drones(&mut self) {
        let drone_ids: Vec<_> = self.drones.keys().cloned().collect();
        for drone_id in drone_ids {
            self.assign_or_return_idle_drone(&drone_id);

            let Some(job) = self.drones.get(&drone_id).and_then(|d| d.job.clone()) else {
                continue;
            };
            self.run_drone_job(&drone_id, job);
        }
    }

    pub(crate) fn ensure_drone_and_job(&mut self, port_id: &str) {
        let Some(port) = self.blocks.get(port_id).cloned() else {
            return;
        };
        if !self.drones.values().any(|drone| drone.home_port == port_id) {
            let id = self.make_id("drone");
            self.drones.insert(
                id.clone(),
                Drone {
                    id,
                    home_port: port_id.to_string(),
                    pos: block_center(&port),
                    battery: 100.0,
                    logic_fuel: 1000,
                    cargo: Inventory::with_capacity(20),
                    state: DroneState::Docked,
                    job: None,
                },
            );
        }

        if !self.tick.is_multiple_of(60) {
            return;
        }
        let Some(dropoff) = self
            .blocks
            .values()
            .find(|b| b.kind == BlockKind::Turret && b.inventory.count(&ItemKind::Ammo) < 10)
            .map(|b| b.id.clone())
        else {
            return;
        };
        if self.delivery_job_exists(&dropoff, &ItemKind::Ammo) {
            return;
        }
        let Some(pickup) = self
            .blocks
            .values()
            .find(|b| {
                matches!(
                    b.kind,
                    BlockKind::Storage | BlockKind::Core | BlockKind::Assembler
                ) && b.inventory.count(&ItemKind::Ammo) >= 5
            })
            .map(|b| b.id.clone())
        else {
            return;
        };
        let job_id = self.make_id("job");
        self.pending_jobs.push(DeliveryJob {
            id: job_id,
            item: ItemKind::Ammo,
            amount: 10,
            pickup,
            dropoff,
            priority: 50,
        });
    }

    fn assign_or_return_idle_drone(&mut self, drone_id: &str) {
        if self
            .drones
            .get(drone_id)
            .and_then(|drone| drone.job.as_ref())
            .is_some()
        {
            return;
        }

        let Some(home_pos) = self
            .drones
            .get(drone_id)
            .and_then(|drone| self.blocks.get(&drone.home_port))
            .map(block_center)
        else {
            return;
        };

        let at_home = self
            .drones
            .get(drone_id)
            .map(|drone| drone.pos.distance(home_pos) <= DOCKING_DISTANCE)
            .unwrap_or(false);
        if !at_home {
            if let Some(drone) = self.drones.get_mut(drone_id) {
                drone.state = DroneState::Returning;
                drone.pos = drone.pos.move_toward(home_pos, DRONE_MOVE_SPEED);
                drone.battery = (drone.battery - 0.05).max(0.0);
            }
            return;
        }

        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.state = DroneState::Docked;
            drone.battery = (drone.battery + 1.0).min(100.0);
            drone.logic_fuel = (drone.logic_fuel + 10).min(1000);
        }

        if let Some(job) = self.take_next_job() {
            if let Some(drone) = self.drones.get_mut(drone_id) {
                drone.job = Some(job);
                drone.state = DroneState::Delivering;
            }
        }
    }

    fn run_drone_job(&mut self, drone_id: &str, job: DeliveryJob) {
        let pickup_pos = self.blocks.get(&job.pickup).map(block_center);
        let dropoff_pos = self.blocks.get(&job.dropoff).map(block_center);
        let Some(dropoff_pos) = dropoff_pos else {
            self.clear_drone_job(drone_id);
            return;
        };

        let mut completed = false;
        if let Some(drone) = self.drones.get_mut(drone_id) {
            drone.battery = (drone.battery - 0.1).max(0.0);
            drone.logic_fuel = drone.logic_fuel.saturating_sub(1);
            if drone.cargo.count(&job.item) == 0 {
                if let Some(pickup_pos) = pickup_pos {
                    if drone.pos.distance(pickup_pos) <= DOCKING_DISTANCE {
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
                        drone.pos = drone.pos.move_toward(pickup_pos, DRONE_MOVE_SPEED);
                    }
                } else {
                    completed = true;
                }
            } else if drone.pos.distance(dropoff_pos) <= DOCKING_DISTANCE {
                let delivered = drone.cargo.remove(&job.item, job.amount);
                if let Some(block) = self.blocks.get_mut(&job.dropoff) {
                    block.inventory.add(job.item.clone(), delivered);
                    block.status = format!("drone delivered {}", job.item.as_str());
                }
                completed = true;
            } else {
                drone.pos = drone.pos.move_toward(dropoff_pos, DRONE_MOVE_SPEED);
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
