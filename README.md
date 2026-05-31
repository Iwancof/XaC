# XaC MVP

XaC is an RTS-as-code / factory-as-code prototype based on `xac_spec.md`.

The current implementation is a vertical slice:

- Tauri v2 desktop shell
- Rust fixed-tick simulation and Tauri IPC
- Wasmtime fuel-backed behavior evaluation shim
- React/TypeScript IDE UI
- PixiJS grid rendering and overlays
- Monaco behavior editor
- Built-in block presets with copy-on-write and fork/edit flow

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
uses a small Wasmtime core module shim to consume real fuel while behavior
source is edited as short pseudo-code/config. The crate boundary is set up so
`xac-wasm` can replace that shim with per-world Component Model bindings.

## UI Direction

XaC should move toward a Mindustry-like construction workflow: a compact
icon-first build palette, clear selected-tool state, fast cancel/rotate/copy
actions near the construction controls, and persistent combat/wave status that
does not obscure the factory grid.
