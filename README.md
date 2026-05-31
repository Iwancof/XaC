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
```

## MVP Notes

The WIT draft lives at `assets/wit/xac.mvp.wit`. The current vertical slice
uses a small Wasmtime core module shim to consume real fuel while behavior
source is edited as short pseudo-code/config. The crate boundary is set up so
`xac-wasm` can replace that shim with per-world Component Model bindings.
