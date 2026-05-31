# XaC MVP

XaC is an RTS-as-code / factory-as-code prototype based on `xac_spec.md`.

The current implementation is a vertical slice:

- Tauri v2 desktop shell
- Rust fixed-tick simulation and Tauri IPC
- WAT-to-Wasm behavior compilation with Wasmtime fuel budgeting
- React/TypeScript IDE UI
- PixiJS grid rendering and overlays
- Monaco behavior editor
- Built-in WAT behavior presets with copy-on-write and fork/edit flow

## Run

```bash
npm install
npm run dev
```

The app creates runtime state under `$XDG_CONFIG_HOME/xac` or `~/.config/xac`.

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
accepts WebAssembly Text (`.wat`) behavior source, compiles it to Wasm with
Wasmtime, and runs each `tick() -> i32` export with fuel derived from the
block's effective network CPU rate. The integer return value is the temporary
MVP action-code ABI; the crate boundary is set up so `xac-wasm` can replace it
with per-world Component Model / WIT bindings.

## UI Direction

XaC should move toward a Mindustry-like construction workflow: a compact
icon-first build palette, clear selected-tool state, fast cancel/rotate/copy
actions near the construction controls, and persistent combat/wave status that
does not obscure the factory grid.
