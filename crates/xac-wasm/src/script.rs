use anyhow::Result;
use xac_core::BehaviorKind;

use super::script_parse::parse_script_source;
use super::script_render::render_script_module;

pub(crate) const ATTACK_POLICY_NEAREST: i32 = 2;
pub(crate) const ATTACK_POLICY_LOWEST_HP: i32 = 3;
pub(crate) const ATTACK_POLICY_RUNNER: i32 = 4;
pub(crate) const ATTACK_POLICY_WIRE_CUTTER: i32 = 5;
pub(crate) const ATTACK_POLICY_ARMORED: i32 = 6;
pub(crate) const ATTACK_POLICY_GRUNT: i32 = 7;

pub(crate) fn is_wat_source(source: &str) -> bool {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with(";;"))
        .is_some_and(|line| line.starts_with("(module"))
}

pub(crate) fn compile_xac_script(kind: BehaviorKind, source: &str) -> Result<String> {
    let parsed = parse_script_source(kind, source)?;
    Ok(render_script_module(
        parsed.imports,
        &parsed.statements,
        &parsed.log_data,
    ))
}
