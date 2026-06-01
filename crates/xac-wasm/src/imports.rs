use std::collections::BTreeSet;
use xac_core::BehaviorKind;

pub(crate) const ALL_BEHAVIOR_KINDS: &[BehaviorKind] = &[
    BehaviorKind::Drill,
    BehaviorKind::Router,
    BehaviorKind::Assembler,
    BehaviorKind::Turret,
    BehaviorKind::DronePort,
    BehaviorKind::CarrierDrone,
];

const DRILL_ONLY: &[BehaviorKind] = &[BehaviorKind::Drill];
const ROUTER_ONLY: &[BehaviorKind] = &[BehaviorKind::Router];
const ASSEMBLER_ONLY: &[BehaviorKind] = &[BehaviorKind::Assembler];
const TURRET_ONLY: &[BehaviorKind] = &[BehaviorKind::Turret];
const DRONE_PORT_ONLY: &[BehaviorKind] = &[BehaviorKind::DronePort];
const CARRIER_DRONE_ONLY: &[BehaviorKind] = &[BehaviorKind::CarrierDrone];

#[derive(Clone, Copy, Debug)]
pub(crate) struct HostImportSpec {
    pub(crate) module: &'static str,
    pub(crate) name: &'static str,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) wit_name: &'static str,
    kinds: &'static [BehaviorKind],
}

impl HostImportSpec {
    pub(crate) fn allowed_for(self, kind: BehaviorKind) -> bool {
        self.kinds.contains(&kind)
    }
}

pub(crate) const HOST_IMPORT_SPECS: &[HostImportSpec] = &[
    HostImportSpec {
        module: "xac:common",
        name: "log",
        wit_name: "log: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:common",
        name: "fuel_remaining",
        wit_name: "fuel-remaining: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:common",
        name: "inventory_count",
        wit_name: "inventory-count: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:common",
        name: "inventory_free",
        wit_name: "inventory-free: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:common",
        name: "stock_count",
        wit_name: "stock-count: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:common",
        name: "stock_capacity",
        wit_name: "stock-capacity: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:common",
        name: "has_space",
        wit_name: "has-space: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:net",
        name: "store_get_i32",
        wit_name: "store-get-i32: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:net",
        name: "store_set_i32",
        wit_name: "store-set-i32: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:net",
        name: "store_delete_i32",
        wit_name: "store-delete-i32: func",
        kinds: ALL_BEHAVIOR_KINDS,
    },
    HostImportSpec {
        module: "xac:drill",
        name: "output_blocked",
        wit_name: "output-blocked: func",
        kinds: DRILL_ONLY,
    },
    HostImportSpec {
        module: "xac:drill",
        name: "mine",
        wit_name: "mine: func",
        kinds: DRILL_ONLY,
    },
    HostImportSpec {
        module: "xac:drill",
        name: "output",
        wit_name: "output: func",
        kinds: DRILL_ONLY,
    },
    HostImportSpec {
        module: "xac:drill",
        name: "ore_kind",
        wit_name: "ore-kind: func",
        kinds: DRILL_ONLY,
    },
    HostImportSpec {
        module: "xac:router",
        name: "push_any",
        wit_name: "push-any: func",
        kinds: ROUTER_ONLY,
    },
    HostImportSpec {
        module: "xac:router",
        name: "push_dir",
        wit_name: "push-dir: func",
        kinds: ROUTER_ONLY,
    },
    HostImportSpec {
        module: "xac:router",
        name: "push_item_dir",
        wit_name: "push-item-dir: func",
        kinds: ROUTER_ONLY,
    },
    HostImportSpec {
        module: "xac:router",
        name: "output_available",
        wit_name: "output-available: func",
        kinds: ROUTER_ONLY,
    },
    HostImportSpec {
        module: "xac:router",
        name: "output_item_available",
        wit_name: "output-item-available: func",
        kinds: ROUTER_ONLY,
    },
    HostImportSpec {
        module: "xac:assembler",
        name: "set_recipe",
        wit_name: "set-recipe: func",
        kinds: ASSEMBLER_ONLY,
    },
    HostImportSpec {
        module: "xac:assembler",
        name: "current_recipe",
        wit_name: "current-recipe: func",
        kinds: ASSEMBLER_ONLY,
    },
    HostImportSpec {
        module: "xac:assembler",
        name: "can_produce",
        wit_name: "can-produce: func",
        kinds: ASSEMBLER_ONLY,
    },
    HostImportSpec {
        module: "xac:assembler",
        name: "input_count",
        wit_name: "input-count: func",
        kinds: ASSEMBLER_ONLY,
    },
    HostImportSpec {
        module: "xac:assembler",
        name: "output_count",
        wit_name: "output-count: func",
        kinds: ASSEMBLER_ONLY,
    },
    HostImportSpec {
        module: "xac:assembler",
        name: "produce",
        wit_name: "produce: func",
        kinds: ASSEMBLER_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "scan_enemies",
        wit_name: "scan-enemies: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "enemy_kind",
        wit_name: "enemy-kind: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "enemy_hp",
        wit_name: "enemy-hp: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "enemy_distance",
        wit_name: "enemy-distance: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "can_attack",
        wit_name: "can-attack: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "attack",
        wit_name: "attack: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "ammo_count",
        wit_name: "ammo-count: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "attack_nearest",
        wit_name: "attack-nearest: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:turret",
        name: "attack_best",
        wit_name: "attack-best: func",
        kinds: TURRET_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "dispatch",
        wit_name: "dispatch: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "stock_count",
        wit_name: "stock-count: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "charge_docked_drones",
        wit_name: "charge-docked-drones: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "docked_drone_count",
        wit_name: "docked-drone-count: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "pending_job_count",
        wit_name: "pending-job-count: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "create_delivery_job",
        wit_name: "create-delivery-job: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone_port",
        name: "dispatch_idle_drones",
        wit_name: "dispatch-idle-drones: func",
        kinds: DRONE_PORT_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "battery_percent",
        wit_name: "battery-percent: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "battery_ratio",
        wit_name: "battery-ratio: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "logic_fuel_remaining",
        wit_name: "logic-fuel-remaining: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "has_job",
        wit_name: "has-job: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "has_pending_job",
        wit_name: "has-pending-job: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "return_to_port",
        wit_name: "return-to-port: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "claim_delivery_job",
        wit_name: "claim-delivery-job: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "deliver",
        wit_name: "deliver: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "move_to",
        wit_name: "move-to: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "load",
        wit_name: "load: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "unload",
        wit_name: "unload: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "cargo_count",
        wit_name: "cargo-count: func",
        kinds: CARRIER_DRONE_ONLY,
    },
    HostImportSpec {
        module: "xac:drone",
        name: "idle",
        wit_name: "idle: func",
        kinds: CARRIER_DRONE_ONLY,
    },
];

pub(crate) fn allowed_host_import(kind: BehaviorKind, module: &str, name: &str) -> bool {
    HOST_IMPORT_SPECS
        .iter()
        .any(|spec| spec.module == module && spec.name == name && spec.allowed_for(kind))
}

pub(crate) fn allowed_worlds(kind: BehaviorKind) -> String {
    HOST_IMPORT_SPECS
        .iter()
        .filter(|spec| spec.allowed_for(kind))
        .map(|spec| spec.module)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}
