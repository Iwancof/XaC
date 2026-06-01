use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use xac_core::CommonTemplate;

use crate::Simulation;

const SETTINGS_TOML: &str = r#"project = "default_project"
autosave = false
"#;

const KEYBINDINGS_TOML: &str = r#"[camera]
pan = "middle_mouse"

[simulation]
toggle_run = "space"
step_tick = "."
"#;

const COMMON_README: &str = r#"# XaC Common Code

Files in this directory are player-owned templates and shared code snippets.
"#;

struct DefaultTemplate {
    id: &'static str,
    display_name: &'static str,
    language: &'static str,
    relative_path: &'static str,
    source: &'static str,
}

const DEFAULT_TEMPLATES: &[DefaultTemplate] = &[
    DefaultTemplate {
        id: "rust_basic_drill",
        display_name: "Rust Basic Drill",
        language: "Rust",
        relative_path: "common/templates/rust/basic_drill.rs",
        source: r#"// XaC Rust template sketch.
// Compile target: wasm32-unknown-unknown with XaC host imports.

#[no_mangle]
pub extern "C" fn tick() {
    if output_blocked() != 0 {
        return;
    }
    mine();
}

extern "C" {
    fn output_blocked() -> i32;
    fn mine() -> i32;
}
"#,
    },
    DefaultTemplate {
        id: "assemblyscript_basic_router",
        display_name: "AssemblyScript Basic Router",
        language: "AssemblyScript",
        relative_path: "common/templates/assemblyscript/basic_router.ts",
        source: r#"// XaC AssemblyScript template sketch.
// Export tick and call XaC host imports from generated bindings.

export function tick(): void {
  if (outputAvailable(1)) {
    push(1);
  }
}

declare function outputAvailable(direction: i32): bool;
declare function push(direction: i32): i32;
"#,
    },
];

pub(crate) fn ensure_user_config(config_root: &Path) -> Result<()> {
    write_if_missing(config_root.join("settings.toml"), SETTINGS_TOML)?;
    write_if_missing(config_root.join("keybindings.toml"), KEYBINDINGS_TOML)?;
    write_if_missing(config_root.join("common/README.md"), COMMON_README)?;
    for template in DEFAULT_TEMPLATES {
        write_if_missing(config_root.join(template.relative_path), template.source)?;
    }
    fs::create_dir_all(config_root.join("common/lib/targeting"))
        .with_context(|| "create common targeting library directory")?;
    fs::create_dir_all(config_root.join("common/lib/logistics"))
        .with_context(|| "create common logistics library directory")?;
    fs::create_dir_all(config_root.join("common/lib/pathing"))
        .with_context(|| "create common pathing library directory")?;
    Ok(())
}

impl Simulation {
    pub fn common_templates(&self) -> Result<Vec<CommonTemplate>> {
        ensure_user_config(&self.config_root)?;
        DEFAULT_TEMPLATES
            .iter()
            .map(|template| {
                let path = self.config_root.join(template.relative_path);
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("read common template {}", path.display()))?;
                Ok(CommonTemplate {
                    id: template.id.to_string(),
                    display_name: template.display_name.to_string(),
                    language: template.language.to_string(),
                    source_path: path.to_string_lossy().to_string(),
                    source,
                })
            })
            .collect()
    }
}

fn write_if_missing(path: PathBuf, contents: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create user config directory {}", parent.display()))?;
    }
    fs::write(&path, contents)
        .with_context(|| format!("write user config file {}", path.display()))?;
    Ok(())
}
