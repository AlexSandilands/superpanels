# Superpanels

> Linux wallpaper manager focused on physical-bezel-aware multi-monitor spanning and folder-driven slideshows.

Single binary. Rust core, Tauri v2 + Svelte 5 GUI. Primary target: Arch / CachyOS on KDE Wayland.

## Install

Not yet on crates.io or the AUR. Build from source:

```sh
git clone https://github.com/alex/superpanels.git
cd superpanels
cargo build --release -p superpanels-cli
# binary at target/release/superpanels
```

`cargo install superpanels` and `yay -S superpanels` are the planned install paths — see [`packaging/README.md`](./packaging/README.md).

## Quick start

```sh
# One-off: tell Superpanels how big each monitor is in mm.
superpanels monitor configure DP-1 --diagonal 27 --aspect 16:9

# Span a panorama across every monitor, bezel-corrected.
superpanels set panorama.jpg

# Preview the crops without touching the wallpaper.
superpanels set panorama.jpg --dry-run
```

The monitor gap (bezel + air-gap between panels) is authored on the GUI canvas (or via `superpanels profile`) and stored on the profile — there is no `--bezel` flag. See [`docs/reference/layout-math.md`](./docs/reference/layout-math.md) for the principle and [`docs/reference/configuration.md`](./docs/reference/configuration.md) for the profile schema.

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
| You want the layout / monitor-gap math | [`docs/reference/layout-math.md`](./docs/reference/layout-math.md) |
| You're touching display detection | [`docs/reference/displays.md`](./docs/reference/displays.md) |
| You're touching a backend | [`docs/reference/backends.md`](./docs/reference/backends.md) |
| You want the config schema | [`docs/reference/configuration.md`](./docs/reference/configuration.md) |
| You're working on the security/IPC surface | [`docs/reference/security.md`](./docs/reference/security.md) |
| You're adding modules / dependencies | [`docs/contributing/architecture.md`](./docs/contributing/architecture.md) |
| You're writing Rust here | [`docs/contributing/style-rust.md`](./docs/contributing/style-rust.md) |
| You're writing TypeScript / Svelte here | [`docs/contributing/style-frontend.md`](./docs/contributing/style-frontend.md) |
| You're writing tests | [`docs/contributing/testing.md`](./docs/contributing/testing.md) |
| You're contributing | [CONTRIBUTING.md](./CONTRIBUTING.md) |
| You're tagging a release | [`packaging/README.md`](./packaging/README.md) |

## Licence

Dual-licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT License](./LICENSE-MIT) at your option. Contributions are accepted under the same terms.
