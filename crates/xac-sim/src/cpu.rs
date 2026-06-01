use crate::{Simulation, TICKS_PER_SECOND};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FuelPolicy {
    pub min_invocation_fuel: u64,
    pub max_bank_seconds: f32,
}

impl Simulation {
    pub(crate) fn grant_fuel_bank(
        &mut self,
        entity_id: &str,
        cpu_rate: f32,
        policy: FuelPolicy,
    ) -> u64 {
        let bank = self.fuel_banks.entry(entity_id.to_string()).or_insert(0.0);
        let max_bank =
            (cpu_rate.max(1.0) * policy.max_bank_seconds).max(policy.min_invocation_fuel as f32);
        *bank = (*bank + cpu_rate / TICKS_PER_SECOND as f32).min(max_bank);
        let available = bank.floor() as u64;
        if available >= policy.min_invocation_fuel {
            available
        } else {
            0
        }
    }

    pub(crate) fn spend_fuel_bank(&mut self, entity_id: &str, fuel_spent: u64) {
        if let Some(bank) = self.fuel_banks.get_mut(entity_id) {
            *bank = (*bank - fuel_spent as f32).max(0.0);
        }
    }
}
