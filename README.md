# XaC MVP

XaC is an RTS-as-code / factory-as-code prototype based on `xac_spec.md`.

The current implementation is a vertical slice:

- Tauri v2 desktop shell
- Rust fixed-tick simulation and Tauri IPC
- XaC Script / WAT-to-Wasm behavior compilation with Wasmtime fuel budgeting
- Host-imported block APIs for behaviors such as drill `mine`
- React/TypeScript IDE UI
- PixiJS grid rendering and overlays
- Monaco behavior editor
- Built-in XaC Script behavior presets with copy-on-write and fork/edit flow

## Run

```bash
npm install
npm run dev
```

The app creates runtime state under `$XDG_CONFIG_HOME/xac` or `~/.config/xac`.
Editable behavior copies are persisted as source files and an index under
`projects/default_project` inside that config root, so the path shown in the
editor points to a real project file.

On Arch Linux, Tauri needs WebKitGTK 4.1:

```bash
sudo pacman -S --needed webkit2gtk-4.1
```

## Useful Checks

```bash
npm run check
cargo test --workspace
npm run test:e2e
```

`npm run test:e2e` starts Vite on `127.0.0.1:5174` with
`VITE_XAC_MOCK_IPC=1`, replaces Tauri IPC with a deterministic in-browser
simulation, and drives the real React/Pixi UI with Playwright. Use this for
concrete user-operation checks such as selecting a block in the right panel,
clicking the map, and opening built-in behavior source without moving the
desktop workspace or launching a Tauri window.

## MVP Notes

The WIT draft lives at `assets/wit/xac.mvp.wit`. The current vertical slice
accepts short XaC Script behavior source such as:

```text
if output_blocked return
mine
```

Drill scripts can also inspect the resource under the drill and explicitly
push stored ore into the block they face:

```text
if ore_kind == ore output ore
```

The backend lowers that source to WebAssembly Text, compiles it to Wasm with
Wasmtime, links a small host API surface such as `xac:drill/mine`, and runs
each `tick()` with fuel derived from the block's effective network CPU rate.
Blocks bank fuel before invoking behavior, so local CPU still works slowly while
wire/core/cpu-node networks reach API-heavy behavior faster. Host API calls also
charge explicit fuel costs, and XaC Script can branch on remaining fuel:

```text
if fuel_remaining > 12 mine
```

Assembler production reads `assets/recipes.toml`; `set_recipe ammo` records an
explicit recipe goal on the block and can build missing intermediate plate from
the same recipe table. Assembler scripts can branch on local inventory before
choosing a recipe:

```text
set_recipe plate
if output_count ammo < 100 set_recipe ammo
if can_produce produce
```

Script can also touch the shared network store through integer keys:

```text
net_set 7 42
if net 7 == 42 attack_best lowest_hp
```

Programmable blocks can read same-network inventory supplied by core, storage,
and other stock-holding blocks:

```text
if stock_count ammo > 50 push ammo east
if has_space ore 10 push ore east
```

Turret scripts can prioritize enemy kinds directly:

```text
if ammo_count > 0 attack_best wire_cutter runner armored nearest
```

The MVP core-Wasm ABI also exposes turret scanning as stable scan indices, so
short scripts can inspect whether visible targets exist and attack a specific
entry from nearest-first scan order:

```text
if scan_enemies > 1 attack 1
if can_attack 0 attack 0
```

Carrier drones also run a built-in XaC Script behavior through the same Wasm
backend. The default drone checks battery and logic fuel, claims a pending ammo
delivery job, delivers it, and returns to its port:

```text
if battery_percent < 25 return_to_port
if battery_percent < 25 return
if logic_fuel_remaining < 100 return_to_port
if logic_fuel_remaining < 100 return
if has_job deliver
if has_job return
if has_pending_job claim_delivery_job
if has_pending_job return
idle
```

Carrier drone code can also bypass the delivery-job helper and command the
physical drone directly. `move_to` advances the free-moving drone toward a tile,
`load` and `unload` transfer cargo to the touched block, and `cargo_count`
branches on the drone's current cargo:

```text
if cargo_count ammo == 0 load ammo 5
if cargo_count ammo > 0 move_to 42 30
```

Drone ports are also code-driven. The default port charges docked drones, checks
network ammo stock, creates a frontline delivery job, and dispatches an idle
carrier:

```text
charge_docked_drones
if stock_count ammo > 50 create_delivery_job ammo 10 frontline
dispatch_idle_drones
```

Routers can make availability checks before moving items:

```text
if output_available east push east
if output_available ammo east push ammo east
```

Raw WebAssembly Text (`.wat`) is still accepted for lower-level tests and power
users. The older `tick() -> i32` action-code ABI remains as a compatibility
fallback, but raw WAT imports are still checked against the selected block
world: drill code cannot import turret APIs, and WASI or other external host
imports are rejected. New built-ins call host imports so player code can
exercise real block capabilities. The current map model supports a 4x4 Core
footprint, wire-backed CPU networks, and non-grid-bound enemy/drone positions.

## UI Direction

XaC should move toward a Mindustry-like construction workflow: a compact
icon-first build palette, clear selected-tool state, fast cancel/rotate/copy
actions near the construction controls, and persistent combat/wave status that
does not obscure the factory grid.
