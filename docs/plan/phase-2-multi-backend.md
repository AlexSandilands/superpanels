# Phase 2 — Multi-backend & slideshow

**Goal.** Works on GNOME and basic X11 setups. Other Wayland compositors covered. Folder-driven slideshow runs in the background.

**Definition of done.**
- [ ] Someone on GNOME 46+ can `cargo install superpanels` and have it work without config changes.
- [ ] On Sway with `swww`, the slideshow rotates a folder of panoramas every 30 minutes across both monitors with the bezel respected.
- [ ] `superpanels daemon` starts in the background, drops to systemd user unit if present, and the slideshow survives a daemon restart with state preserved.

## 2.1 Additional backends
- [x] `GnomeBackend`: `gsettings set org.gnome.desktop.background picture-uri[-dark] file://...`. Multi-monitor strategy: composite the per-monitor crops to a single image of the spanning canvas and set that as the wallpaper. Cap composite resolution at 8K long-edge to bound memory; downscale if the canvas exceeds that.
- [x] `HyprlandBackend`: `hyprctl hyprpaper preload` + `hyprctl hyprpaper wallpaper "MONITOR,PATH"` per monitor. If `hyprpaper` isn't running, return a clear error with the start command in the message.
- [x] `SwayBackend`: detect `swww` first, fall back to `swaybg`. `swww img --outputs MON path` for per-monitor.
- [x] `FehBackend`: `feh --bg-fill IMAGE1 IMAGE2 ...`. X11 only.
- [x] `CustomBackend`: `{image_N}` and `{monitor_N}` substitution in a configured template, executed with the standard subprocess rules.
- [x] Auto-detection ordering per SPEC §10.2; pinning via config.

## 2.2 Additional detectors
- [x] `WlrRandrDetector` parsing `wlr-randr --json`.
- [x] `HyprctlDetector` parsing `hyprctl monitors -j`.
- [x] `XrandrDetector` parsing `xrandr --verbose` for X11. Physical-mm extraction.

## 2.3 `slideshow.rs`
- [x] `SlideshowConfig` per SPEC §9.2.
- [x] Picker: respects `sort`, `recent_history_size`, `ImageFilters`.
- [x] State persisted to `$XDG_STATE_HOME/superpanels/state.json` (history, current index, paused).
- [x] Skip-on-unavailable handling.
- [x] Tests: a fake clock + a fake folder source proves rotation, history suppression, and resume-after-restart.

## 2.4 Folder source & `library.rs` (lite)
- [x] Folder scanning (recursive flag) producing `Vec<LibraryEntry>` with resolution, aspect ratio, mtime.
- [x] `notify`-backed FS watch on configured roots; incremental index updates.
- [x] Rayon-parallel initial scan with a progress callback (for the GUI to consume later).
- [x] No SQLite yet — flat in-memory index serialised to its own file at `$XDG_STATE_HOME/superpanels/library-index.json`. **Not** mixed into `state.json` (different write cadence, different migration concerns; the library index is rebuildable from disk while `state.json` is not). SQLite replaces this in Phase 4b with tags.

## 2.5 `daemon/` binary
- [x] `superpanels daemon [--foreground]`.
- [x] Single-instance lock at `$XDG_RUNTIME_DIR/superpanels/daemon.sock`.
- [x] IPC server (length-prefixed JSON) handling: `apply_profile`, `slideshow_next`, `slideshow_prev`, `slideshow_pause`, `redetect`, `current_state`.
- [x] Tokio runtime; slideshow timer via `tokio::time::interval`; cancel-safe.
- [x] FS watcher hooked to library updates.
- [x] Logout-friendly: traps `SIGTERM`, persists state, exits cleanly.
- [x] Optional systemd user unit file generated on demand: `superpanels daemon --install-unit`.

## 2.6 CLI ↔ daemon
- [x] `superpanels set` etc. detect a running daemon and forward via IPC.
- [x] `--no-daemon` runs in-process unconditionally (useful for SSH / scripting).
- [x] `superpanels next` / `prev` / `pause` / `resume` IPC-only commands (require a daemon, friendly error otherwise).
- [x] `superpanels profile apply` / `list` / `delete` / `rename` / `export` / `import`.

## 2.7 Schedules
- [ ] `Schedule::Daily { at, profile }` (timezone: system local).
- [ ] `Schedule::Sunset { offset, profile }` — uses a small algorithmic sunrise/sunset crate; lat/long configured manually (no IP geolocation).
- [ ] `Schedule::Cron(expr)` — `cron`-crate parsing.
- [ ] Daemon evaluates schedules on a 60-second tick.
- [ ] Tests: a fake clock + a fake config drives every schedule kind.

## 2.8 Save-as profile
- [ ] `superpanels set --save-as NAME [...args]` writes a profile and applies it.

**Risks for this phase.**
- Hyprland's `hyprctl` JSON shape changes between minor versions. Track latest stable; capture multi-version fixtures.
- GNOME's "one image stretched across the desktop" behaviour can interact oddly with HiDPI; the composite-then-scale path needs visual verification.
- IPC socket permission handling on non-`XDG_RUNTIME_DIR` systems. Fall back to `/tmp/superpanels-$UID/daemon.sock` with `0700` mode.
