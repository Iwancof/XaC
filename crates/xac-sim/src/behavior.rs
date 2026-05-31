use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use xac_core::{BehaviorId, BehaviorSummary, BlockKind};
use xac_wasm::hash_behavior_source;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorPackage {
    pub summary: BehaviorSummary,
    pub source: String,
    pub wasm_hash: Option<String>,
}

pub fn builtin_behaviors() -> BTreeMap<BehaviorId, BehaviorPackage> {
    let mut packages = BTreeMap::new();
    for (id, display_name, base_kind, world, source_path, source) in [
        (
            "builtin.drill.basic",
            "Basic Drill",
            BlockKind::Drill,
            "drill-behavior",
            "assets/builtin/drill/basic.xac",
            include_str!("../../../assets/builtin/drill/basic.xac"),
        ),
        (
            "builtin.router.basic",
            "Basic Router",
            BlockKind::Router,
            "router-behavior",
            "assets/builtin/router/basic.xac",
            include_str!("../../../assets/builtin/router/basic.xac"),
        ),
        (
            "builtin.router.ammo_east",
            "Ammo East Router",
            BlockKind::Router,
            "router-behavior",
            "assets/builtin/router/ammo_east.xac",
            include_str!("../../../assets/builtin/router/ammo_east.xac"),
        ),
        (
            "builtin.assembler.basic",
            "Basic Assembler",
            BlockKind::Assembler,
            "assembler-behavior",
            "assets/builtin/assembler/basic.xac",
            include_str!("../../../assets/builtin/assembler/basic.xac"),
        ),
        (
            "builtin.turret.basic",
            "Basic Turret",
            BlockKind::Turret,
            "turret-behavior",
            "assets/builtin/turret/basic.xac",
            include_str!("../../../assets/builtin/turret/basic.xac"),
        ),
        (
            "builtin.turret.priority",
            "Priority Turret",
            BlockKind::Turret,
            "turret-behavior",
            "assets/builtin/turret/priority.xac",
            include_str!("../../../assets/builtin/turret/priority.xac"),
        ),
        (
            "builtin.drone_port.basic",
            "Basic Drone Port",
            BlockKind::DronePort,
            "drone-port-behavior",
            "assets/builtin/drone_port/basic.xac",
            include_str!("../../../assets/builtin/drone_port/basic.xac"),
        ),
    ] {
        let id = id.to_string();
        packages.insert(
            id.clone(),
            BehaviorPackage {
                summary: BehaviorSummary {
                    id: id.clone(),
                    display_name: display_name.to_string(),
                    base_kind,
                    world: world.to_string(),
                    builtin: true,
                    used_by: 0,
                    source_path: source_path.to_string(),
                    build_status: "builtin".to_string(),
                },
                source: source.to_string(),
                wasm_hash: Some(
                    hash_behavior_source(base_kind, source).expect("valid builtin source"),
                ),
            },
        );
    }
    packages
}
