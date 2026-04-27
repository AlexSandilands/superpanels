# 5. Application architecture

## 5.1 Process model

Superpanels ships as a single binary with multiple personalities, selected by subcommand:

| Personality | Invocation | What it does |
|---|---|---|
| One-shot CLI | `superpanels set …` | Apply a wallpaper, exit. |
| Detector | `superpanels detect [--json]` | Print monitor layout, exit. |
| Profile actions | `superpanels profile …` | List/apply/edit/delete profiles, exit. |
| Daemon | `superpanels daemon` | Background process: slideshow timer, schedule triggers, FS watch, IPC server. No UI. |
| GUI | `superpanels gui` | Tauri window + system tray. Spawns/connects to daemon for background work. |

Single-binary keeps packaging trivial. Each subcommand is dispatched in `main.rs`; the rest is library code.

## 5.2 Single-instance behaviour

- The daemon and GUI are mutually-aware: at most one daemon runs per user session. The lock is a Unix domain socket at `$XDG_RUNTIME_DIR/superpanels/daemon.sock`.
- If the user runs `superpanels gui` and a daemon is already running, the GUI connects to it over the IPC socket. If no daemon is running, the GUI spawns one as a child and supervises it.
- Running `superpanels gui` twice raises the existing window (via the IPC socket) instead of opening a second window.

## 5.3 IPC protocol

Length-prefixed JSON over the Unix socket. Versioned (`{"v": 1, "method": "...", "params": {...}}`). Methods mirror the Tauri commands so the GUI's command handler is a thin pass-through. The CLI also speaks IPC: `superpanels set` running while a daemon is up sends a `set` request to the daemon rather than re-detecting + re-applying itself, so the daemon's state (current image, slideshow position) stays consistent.

If the daemon isn't running, the CLI does the work in-process and exits — no daemon required for one-shot use.

## 5.4 Library / wrapper layout

```
superpanels/
├── Cargo.toml                 ← workspace root
├── crates/
│   ├── superpanels-core/      ← pure-Rust library (no UI, no IPC, fully testable)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── display/       ← Monitor model + detection orchestration
│   │       │   ├── mod.rs
│   │       │   ├── kscreen.rs
│   │       │   ├── wlr_randr.rs
│   │       │   ├── hyprctl.rs
│   │       │   └── xrandr.rs
│   │       ├── layout.rs      ← bezel math, CropSpec computation
│   │       ├── image.rs       ← load, scale, crop, rotate, save_temp
│   │       ├── library.rs     ← folder scanning, filtering, thumbnails, history
│   │       ├── slideshow.rs   ← rotation logic, history, smart selection
│   │       ├── schedule.rs    ← time-of-day triggers (cron-ish)
│   │       ├── config.rs      ← TOML config + profiles, serde
│   │       ├── state.rs       ← runtime state persistence (current wallpaper, etc.)
│   │       └── backends/
│   │           ├── mod.rs
│   │           ├── kde.rs
│   │           ├── gnome.rs
│   │           ├── sway.rs
│   │           ├── hyprland.rs
│   │           ├── feh.rs
│   │           └── custom.rs
│   ├── superpanels-cli/       ← clap-based CLI binary (thin wrapper around core)
│   │   └── src/main.rs
│   ├── superpanels-daemon/    ← daemon binary (timers, IPC server, FS watch)
│   │   └── src/main.rs
│   └── superpanels-gui/       ← Tauri shell (only built with --features gui)
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       └── src/main.rs
└── ui/                        ← Svelte 5 frontend
    ├── package.json
    ├── vite.config.ts
    └── src/
        ├── App.svelte
        ├── lib/
        │   ├── canvas/
        │   │   ├── MonitorCanvas.svelte
        │   │   ├── canvas-render.ts
        │   │   └── canvas-interaction.ts
        │   ├── library/
        │   │   ├── LibraryGrid.svelte
        │   │   ├── ThumbnailTile.svelte
        │   │   └── LibraryFilters.svelte
        │   ├── profile/
        │   │   ├── ProfileList.svelte
        │   │   ├── BezelControls.svelte
        │   │   └── FitControls.svelte
        │   ├── settings/
        │   │   └── SettingsPanel.svelte
        │   └── ui/             ← reusable buttons, toasts, modals
        └── stores/
            ├── profile.ts
            ├── monitors.ts
            ├── library.ts
            └── toast.ts
```

`superpanels-cli` and `superpanels-gui` are technically separate binaries but the published artefact is a single combined binary that dispatches by subcommand — Cargo features (`gui`, `cli-only`) gate which subcommands are compiled in. Distros that only want the CLI build with `--no-default-features --features cli-only`.

## 5.5 Threading

- The core library is `Send + Sync`-friendly; long-running ops (image processing, FS scan) are on a Tokio runtime in the daemon.
- The Tauri GUI invokes core via `tauri::async_runtime::spawn_blocking` for image work to keep the UI thread free.
- The slideshow timer uses `tokio::time::interval` rather than thread-sleep, so it's cancellation-safe.
