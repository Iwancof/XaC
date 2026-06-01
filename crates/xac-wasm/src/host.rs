use anyhow::Result;
use wasmtime::Linker;

use super::host_assembler::define_assembler_imports;
use super::host_common::define_common_imports;
use super::host_drill::define_drill_imports;
use super::host_drone::define_drone_imports;
use super::host_drone_port::define_drone_port_imports;
use super::host_net::define_net_imports;
use super::host_router::define_router_imports;
use super::host_turret::define_turret_imports;
use super::BehaviorHostState;

pub(super) fn define_host_imports(linker: &mut Linker<BehaviorHostState>) -> Result<()> {
    define_common_imports(linker)?;
    define_drill_imports(linker)?;
    define_router_imports(linker)?;
    define_assembler_imports(linker)?;
    define_turret_imports(linker)?;
    define_drone_port_imports(linker)?;
    define_drone_imports(linker)?;
    define_net_imports(linker)?;
    Ok(())
}
