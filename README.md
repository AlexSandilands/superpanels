# Superpanels

> Linux wallpaper manager focused on physical-bezel-aware multi-monitor spanning and folder-driven slideshows.

**Status: Phase 3 (Tauri shell & tray).** The Rust core, `superpanels` CLI, daemon, and Tauri GUI with system tray are working; polish and packaging follow in [`docs/plan/`](./docs/plan/) Phases 4–5.

Single binary. Rust core, Tauri v2 + Svelte 5 GUI. Primary target: Arch / CachyOS on KDE Wayland.

## Install

Not yet on crates.io or the AUR. Build from source:

```sh
git clone https://github.com/alex/superpanels.git
cd superpanels
cargo build --release -p superpanels-cli
# binary at target/release/superpanels
```

Once Phase 5 ships, `cargo install superpanels` and `yay -S superpanels` will be the supported install paths.

## Quick start

```sh
# Tell Superpanels how big each monitor is in mm (one-off, see docs/spec/06-detection.md).
superpanels monitor configure DP-1 --diagonal 27in --aspect 16:9

# Span a panorama across every monitor, bezel-corrected.
superpanels set panorama.jpg --bezel-h 8

# Or preview the crops without touching the wallpaper.
superpanels set panorama.jpg --bezel-h 8 --dry-run
```

See [`docs/spec/`](./docs/spec/) for the full design (split by section) and [`superpanels --help`](./crates/superpanels-cli/src/main.rs) for the current command surface.

## Running locally

The CLI runs without the daemon (in-process fallback). Run the daemon when you want the slideshow timer, file watcher, or tray to keep working in the background.

**Daemon — foreground with logs (Ctrl-C to stop):**

```sh
cargo run -p superpanels-daemon -- --foreground -v
```

`-v` is debug, `-vv` is trace. From another shell, stop a backgrounded daemon with `pkill -INT superpanels-daemon` (avoid `-9` — it skips the state-persist step).

**GUI — open the window with tray icon:**

Tauri loads the frontend from `devUrl` in debug builds and from the bundled `ui/dist` in release builds, so the right command depends on what you're doing.

*Smoke test (release build, bundled UI, no dev server needed):*

```sh
npm --prefix ui run build
cargo run -p superpanels-gui --release
```

*Frontend iteration (debug build + Vite HMR, two terminals):*

```sh
# terminal 1 — Vite dev server on http://localhost:5173
npm --prefix ui run dev

# terminal 2 — Tauri shell (debug)
cargo run -p superpanels-gui
```

Start the daemon first if you want the tray's profile-switch menu to reflect live state.

> **Wayland note.** WebKitGTK 2.46+ ships with a DMABUF renderer that crashes on several common stacks (NVIDIA, recent Mesa + Plasma 6) with `Gdk-Message: Error 71 (Protocol error)`. The repo's `.cargo/config.toml` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` for every `cargo run`, and the autostart `.desktop` file written by the GUI mirrors the same prefix in its `Exec=` line. No manual env juggling needed.

[`cargo-tauri`](https://tauri.app/start/) (`cargo install tauri-cli --version '^2.0.0' --locked`) wraps the two-terminal HMR flow into `cargo tauri dev --manifest-path crates/superpanels-gui/Cargo.toml`. Optional — not needed for smoke testing.

Tauri OS prerequisites are listed in [CONTRIBUTING.md](./CONTRIBUTING.md).

## What it does

- Spans a panorama across multiple monitors with the **physical bezel gap** accounted for, so the image stays continuous as the eye sees it across the desk.
- Drives a folder-of-wallpapers slideshow on a schedule, with smart aspect-ratio filtering and recent-history suppression.
- Ships a CLI for headless / scripted use and an optional GUI with a live, scaled, bezel-accurate monitor preview canvas.
- Supports KDE, GNOME, Sway, Hyprland, X11/feh, and a custom-command escape hatch.

## Documentation

| Read this when | Doc |
|---|---|
| You want to know what's being built | [`docs/spec/`](./docs/spec/) (split by section) |
| You want to know what's next | [`docs/plan/`](./docs/plan/) (split by phase) |
| You want to contribute | [CONTRIBUTING.md](./CONTRIBUTING.md) |
| You're writing Rust here | [docs/style-rust.md](./docs/style-rust.md) |
| You're writing TypeScript / Svelte here | [docs/style-frontend.md](./docs/style-frontend.md) |
| You're adding modules / dependencies | [docs/architecture.md](./docs/architecture.md) |
| You're writing tests | [docs/testing.md](./docs/testing.md) |

## Licence

Dual-licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT License](./LICENSE-MIT) at your option. Contributions are accepted under the same terms.
