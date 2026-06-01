use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use xac_core::{BehaviorId, BehaviorKind, BehaviorSummary};
use xac_wasm::{hash_behavior_source, CompiledBehavior};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BehaviorPackage {
    pub summary: BehaviorSummary,
    pub source: String,
    pub wasm_hash: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct ProjectBehaviorIndex {
    behavior: Vec<ProjectBehaviorRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProjectBehaviorRecord {
    id: BehaviorId,
    display_name: String,
    base_kind: BehaviorKind,
    world: String,
    source_path: String,
    build_status: String,
    wasm_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WasmCacheMetadata {
    behavior_id: BehaviorId,
    base_kind: BehaviorKind,
    world: String,
    source_path: String,
    wasm_hash: String,
}

const PROJECT_INDEX_PATH: &str = "projects/default_project/behaviors.toml";

pub fn load_behaviors(config_root: &Path) -> Result<BTreeMap<BehaviorId, BehaviorPackage>> {
    let mut behaviors = builtin_behaviors();
    let index_path = project_behavior_index_path(config_root);
    if !index_path.exists() {
        return Ok(behaviors);
    }

    let index_source = fs::read_to_string(&index_path)
        .with_context(|| format!("read project behavior index {}", index_path.display()))?;
    let index: ProjectBehaviorIndex = toml::from_str(&index_source)
        .with_context(|| format!("parse project behavior index {}", index_path.display()))?;

    for record in index.behavior {
        let source_path = PathBuf::from(&record.source_path);
        let source = fs::read_to_string(&source_path)
            .with_context(|| format!("read behavior source {}", source_path.display()))?;
        let id = record.id.clone();
        behaviors.insert(
            id.clone(),
            BehaviorPackage {
                summary: BehaviorSummary {
                    id,
                    display_name: record.display_name,
                    base_kind: record.base_kind,
                    world: record.world,
                    builtin: false,
                    used_by: 0,
                    source_path: record.source_path,
                    build_status: record.build_status,
                },
                source,
                wasm_hash: record.wasm_hash,
            },
        );
    }

    Ok(behaviors)
}

pub fn project_behavior_source_path(config_root: &Path, behavior_id: &str) -> PathBuf {
    config_root
        .join("projects/default_project/blocks")
        .join(behavior_id)
        .join("src/behavior.xac")
}

pub fn persist_project_behavior_source(package: &BehaviorPackage) -> Result<()> {
    if package.summary.builtin {
        return Ok(());
    }
    let path = PathBuf::from(&package.summary.source_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create behavior source dir {}", parent.display()))?;
    }
    fs::write(&path, &package.source)
        .with_context(|| format!("write behavior source {}", path.display()))?;
    Ok(())
}

pub fn persist_project_behavior_index(
    config_root: &Path,
    behaviors: &BTreeMap<BehaviorId, BehaviorPackage>,
) -> Result<()> {
    let index_path = project_behavior_index_path(config_root);
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create behavior index dir {}", parent.display()))?;
    }
    let index = ProjectBehaviorIndex {
        behavior: behaviors
            .values()
            .filter(|package| !package.summary.builtin)
            .map(|package| ProjectBehaviorRecord {
                id: package.summary.id.clone(),
                display_name: package.summary.display_name.clone(),
                base_kind: package.summary.base_kind,
                world: package.summary.world.clone(),
                source_path: package.summary.source_path.clone(),
                build_status: package.summary.build_status.clone(),
                wasm_hash: package.wasm_hash.clone(),
            })
            .collect(),
    };
    let index_source =
        toml::to_string_pretty(&index).context("serialize project behavior index")?;
    fs::write(&index_path, index_source)
        .with_context(|| format!("write project behavior index {}", index_path.display()))?;
    Ok(())
}

pub fn persist_compiled_behavior_cache(
    config_root: &Path,
    package: &BehaviorPackage,
    compiled: &CompiledBehavior,
) -> Result<()> {
    let cache_dir = config_root.join("cache/wasm");
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("create wasm cache dir {}", cache_dir.display()))?;

    let wasm_hash = compiled.wasm_hash();
    let wasm_path = cache_dir.join(format!("{wasm_hash}.wasm"));
    fs::write(&wasm_path, compiled.wasm_bytes())
        .with_context(|| format!("write cached behavior wasm {}", wasm_path.display()))?;

    let metadata = WasmCacheMetadata {
        behavior_id: package.summary.id.clone(),
        base_kind: package.summary.base_kind,
        world: package.summary.world.clone(),
        source_path: package.summary.source_path.clone(),
        wasm_hash: wasm_hash.to_string(),
    };
    let metadata_path = cache_dir.join(format!("{wasm_hash}.metadata.toml"));
    let metadata_source =
        toml::to_string_pretty(&metadata).context("serialize wasm cache metadata")?;
    fs::write(&metadata_path, metadata_source)
        .with_context(|| format!("write cached behavior metadata {}", metadata_path.display()))?;

    Ok(())
}

fn project_behavior_index_path(config_root: &Path) -> PathBuf {
    config_root.join(PROJECT_INDEX_PATH)
}

pub fn builtin_behaviors() -> BTreeMap<BehaviorId, BehaviorPackage> {
    let mut packages = BTreeMap::new();
    for (id, display_name, base_kind, world, source_path, source) in [
        (
            "builtin.drill.basic",
            "Basic Drill",
            BehaviorKind::Drill,
            "drill-behavior",
            "assets/builtin/drill/basic.xac",
            include_str!("../../../assets/builtin/drill/basic.xac"),
        ),
        (
            "builtin.router.basic",
            "Basic Router",
            BehaviorKind::Router,
            "router-behavior",
            "assets/builtin/router/basic.xac",
            include_str!("../../../assets/builtin/router/basic.xac"),
        ),
        (
            "builtin.router.ammo_east",
            "Ammo East Router",
            BehaviorKind::Router,
            "router-behavior",
            "assets/builtin/router/ammo_east.xac",
            include_str!("../../../assets/builtin/router/ammo_east.xac"),
        ),
        (
            "builtin.assembler.basic",
            "Basic Assembler",
            BehaviorKind::Assembler,
            "assembler-behavior",
            "assets/builtin/assembler/basic.xac",
            include_str!("../../../assets/builtin/assembler/basic.xac"),
        ),
        (
            "builtin.turret.basic",
            "Basic Turret",
            BehaviorKind::Turret,
            "turret-behavior",
            "assets/builtin/turret/basic.xac",
            include_str!("../../../assets/builtin/turret/basic.xac"),
        ),
        (
            "builtin.turret.priority",
            "Priority Turret",
            BehaviorKind::Turret,
            "turret-behavior",
            "assets/builtin/turret/priority.xac",
            include_str!("../../../assets/builtin/turret/priority.xac"),
        ),
        (
            "builtin.drone_port.basic",
            "Basic Drone Port",
            BehaviorKind::DronePort,
            "drone-port-behavior",
            "assets/builtin/drone_port/basic.xac",
            include_str!("../../../assets/builtin/drone_port/basic.xac"),
        ),
        (
            "builtin.carrier_drone.basic",
            "Basic Carrier Drone",
            BehaviorKind::CarrierDrone,
            "carrier-drone-behavior",
            "assets/builtin/carrier_drone/basic.xac",
            include_str!("../../../assets/builtin/carrier_drone/basic.xac"),
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
