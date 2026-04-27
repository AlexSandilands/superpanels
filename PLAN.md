# Superpanels — Development Plan

> Companion to [SPEC.md](./SPEC.md). The spec describes *what* we're building; this describes *how* and *in what order*, with definitions of done so we know when each phase is finished.

---

## Table of contents

1. [Guiding principles](#guiding-principles)
2. [Phase map](#phase-map)
3. [Phase 0 — Spike](#phase-0--spike-derisk-the-hard-bits-1-2-days)
4. [Phase 1 — Core CLI MVP](#phase-1--core-cli-mvp)
5. [Phase 2 — Multi-backend & slideshow](#phase-2--multi-backend--slideshow)
6. [Phase 3 — Tauri shell & tray](#phase-3--tauri-shell--tray)
7. [Phase 4a — Canvas interaction](#phase-4a--canvas-interaction)
8. [Phase 4b — Library + SQLite](#phase-4b--library--sqlite)
9. [Phase 4c — Polish & accessibility](#phase-4c--polish--accessibility)
10. [Phase 5 — Packaging & first release](#phase-5--packaging--first-release-v01)
11. [Phase 6 — Stabilisation toward 1.0](#phase-6--stabilisation-toward-10)
10. [Cross-cutting concerns](#cross-cutting-concerns)
11. [Risk register](#risk-register)
12. [Decisions to make early](#decisions-to-make-early)
13. [First commits playbook](#first-commits-playbook)

---

## Guiding principles

- **Build vertically, not horizontally.** Each phase ships something that works end-to-end on at least one real desktop. Half-finished features stay on a branch.
- **Derisk first.** The hard bits (bezel math correctness; KDE D-Bus reliability) get prototyped in Phase 0, before architecture commitments.
- **Pure-Rust core, thin wrappers.** Anything in `superpanels-core` is unit-testable without spawning processes, opening windows, or touching the file system beyond a tempdir. CLI and GUI are dispatchers around the core API.
- **No placeholder code.** If it's merged, it works and is tested. `todo!()` only behind explicit feature flags.
- **No premature abstractions.** Three near-identical backend modules is fine; an abstract "BackendBuilder" framework is not.
- **Small, focused commits.** Diff reviewable in one sitting. The spec is the design doc; commit messages are the changelog.
- **Slick is a requirement.** GUI polish is scoped from Phase 3; it's not a "later" task.

---

## Phase map

| Phase | Goal | Ship-ready demo | Target version |
|---|---|---|---|
| 0 | Spike & derisk | A throwaway binary that prints correct crops for a 3-monitor KDE setup | — |
| 1 | Core CLI MVP on KDE | `superpanels set pano.jpg` works, with bezel correction | v0.1.0 (CLI only, KDE only) |
| 2 | Multi-backend + slideshow | Works on GNOME, Sway, Hyprland, X11/feh; folder-driven rotation | v0.2.x |
| 3 | Tauri shell + tray | `superpanels gui` window + system tray; profile switching | v0.3.x |
| 4a | Canvas interaction | Drag-to-offset + live bezel sliders; ≥ 60 fps preview | v0.4.x |
| 4b | Library + SQLite | Library grid + thumbnails + tags + favourites | v0.5.x |
| 4c | Polish & accessibility | Onboarding, theming, keyboard shortcuts, a11y audit | v0.6.x |
| 5 | Packaging & v0.7 release | AUR + crates.io + GitHub release artefacts | v0.7.0 |
| 6 | Stabilise toward 1.0 | Schema freeze, docs, perf budget enforced | v0.8 → v1.0.0 |

Rough effort: Phase 0–2 is the unglamorous engine work (the majority of the value). Phase 3–4 is the visible payoff. Phase 5–6 is hardening.

The version trajectory is *intent*, not contract. Pre-1.0 minor bumps may include breaking config changes; the changelog is explicit. The schema is frozen at v1.0; before then we accept the migration cost rather than freezing too early.

---

## Phase 0 — Spike: derisk the hard bits (1–2 days)

**Goal.** Validate the two hardest assumptions in the spec before committing to architecture.

**Deliverable.** A *throwaway* standalone Rust binary (its own scratch directory outside this repo, or a `rust-script` single-file program — the workspace doesn't exist yet) that:
1. Reads `kscreen-doctor -o` and parses it into `Vec<Monitor>` including physical sizes in mm.
2. Computes `Vec<CropSpec>` for an arbitrary input image and the detected layout.
3. Prints the result in JSON.

This validates: the parser is feasible; physical-mm data is actually present; the bezel math gives sensible numbers on a real layout. **No GUI, no D-Bus, no apply.**

> Note: `examples/spike.rs` would require an existing Cargo workspace. We don't have one yet — Phase 1.1 creates it. Keep the spike entirely separate so no spike code can drift into Phase 1.

**Definition of done.**
- [ ] kscreen-doctor parser handles a real 3-monitor KDE layout from the dev machine.
- [ ] Bezel math run by hand (calculator) for that layout matches the program's output.
- [ ] The spike code is *deleted* (or moved to `examples/`) before Phase 1 starts. Do not let spike code become production code.

**Outputs that survive into Phase 1.**
- The captured `kscreen-doctor` output sample → moves to `tests/fixtures/display/` for the real parser.
- The list of monitor-layout edge cases the spike surfaced → opens a GitHub issue list.

---

## Phase 1 — Core CLI MVP

**Goal.** `superpanels set panorama.jpg` works on KDE Wayland and produces a correctly bezel-spliced wallpaper.

**Definition of done.**
- [ ] On a fresh CachyOS KDE install with `cargo install superpanels`, a panorama spans three monitors with bezel correction on first try. *(Bezel-correct spanning verified locally on Plasma 6.6.4 / Wayland; the `cargo install` half is gated on the crates.io publish in Phase 5.2.)*
- [x] `superpanels detect` prints the monitor table and exits zero.
- [x] All bezel-math edge cases listed in SPEC §4.5 have passing tests.
- [x] `cargo clippy --all-features -- -D warnings` is clean.
- [x] `cargo test` runs in < 30 s.

### 1.1 Workspace scaffold
- [x] `cargo new --lib superpanels-core` inside a workspace `Cargo.toml`.
- [x] Add `superpanels-cli` crate as a separate binary; `superpanels-cli` depends on `superpanels-core`.
- [x] Workspace deps: `image`, `anyhow`, `thiserror`, `serde`, `serde_json`, `toml`, `toml_edit` (round-trip-with-comments writes), `clap` (with `derive`), `tracing`, `tracing-subscriber`.
- [x] `rust-toolchain.toml` pinning to a stable Rust version (recent stable, no nightly).
- [x] `rustfmt.toml`, `clippy.toml` (lints config), `.editorconfig`.
- [x] `#![forbid(unsafe_code)]` in every crate.
- [x] `.gitignore` (target/, *.bak, dist/, ui/node_modules/, ui/dist/).
- [x] `LICENSE` (MIT or Apache-2.0/MIT dual; pick early).
- [x] Minimal `README.md` with one-line description + "still in development" badge.
- [x] CI skeleton (`.github/workflows/ci.yml`) running `cargo test`, `clippy`, `fmt --check` on push.

### 1.2 `display/` — KDE detection
- [x] `Monitor` struct per SPEC §3.1, `serde::Serialize`. Note: `physical_size_mm: Option<(u32, u32)>` and `ppi: Option<f64>` — both `None` until merged with config (§1.6).
- [x] `Rotation` enum, `serde`-friendly. Includes the KDE numeric mapping (1=None, 2=Right, 4=Inverted, 8=Left) — verify via spike fixture.
- [x] `MonitorRef { stable_id, name }` per SPEC §6.4.
- [x] `DisplayDetector` trait + `Availability` + `DetectError` per SPEC §6.1.
- [x] `KscreenDoctorDetector` — spawns subprocess with `NO_COLOR=1` env (fixture `tests/fixtures/display/kscreen-3-monitors.txt` has ANSI escapes if not stripped; setting the env avoids the regex), timeout 5 s, parses output. Extracts per-output UUID as `stable_id`. **Does not extract physical mm** — kscreen-doctor doesn't expose it.
  - Fixture already captured for the 3×27" 2560×1440 case. Add a single-monitor fixture from any other system before merging.
  - Tests run against fixtures, *not* a live system.
- [x] Manual override parser for `--monitors` per SPEC §6.2 (layout-only and full-with-mm forms both supported).
- [x] `detect()` orchestrator: try KDE detector, then manual override, then bail with the friendly error from SPEC §6.
- [x] **Config merge step:** after detection, walk `[[monitor]]` blocks from §14.1 config and populate `physical_size_mm` (matching by `stable_id` first, then `name`). Compute `ppi` for monitors that got a size.
- [x] `superpanels detect` and `superpanels detect --json` CLI wired up. Output indicates which monitors are missing physical_mm.
- [x] `--debug` flag prints attempted detectors + their stderr + which `Availability` variant each returned.

### 1.3 `layout.rs` — bezel math
- [x] `BezelConfig`, `CropSpec`, `Rect`, `FitMode`, `LayoutError` types per SPEC §3.
- [x] `compute_crop_specs(monitors, bezels, image_size, fit) -> Result<Vec<CropSpec>, LayoutError>`.
- [x] **Pre-flight check:** any monitor with `physical_size_mm: None` triggers `LayoutError::PhysicalSizeMissing { monitors: Vec<MonitorRef> }` so callers can prompt the user. Test this explicitly.
- [x] Single row, identical monitors, zero bezel — trivial test.
- [x] Single row, identical monitors, uniform bezel — the canonical test.
- [x] Single row, mixed PPI — verify reference-PPI normalisation.
- [x] Mixed orientation (one portrait monitor) — rotation handling.
- [x] 2×2 grid — vertical bezel logic.
- [x] Single-monitor degenerate case.
- [x] Property tests with `proptest`:
  - Sum of crop widths in pixel space equals image width × scaling, modulo gap.
  - No two crops overlap.
  - Every monitor receives exactly one crop.

### 1.4 `image.rs` — processing
- [x] `load(path) -> Result<DynamicImage>` with friendly error on unsupported format / bad file.
- [x] `scale_to_fit(img, target, FitMode) -> DynamicImage`. Lanczos3 default.
- [x] `crop(img, rect) -> DynamicImage` with bounds-check error.
- [x] `rotate(img, Rotation) -> DynamicImage` (uses `image::imageops::rotate90` etc.).
- [x] `save_temp(img, name) -> Result<PathBuf>` writing to `$XDG_CACHE_HOME/superpanels/temp/`.
- [x] Clear temp dir at the start of each run (atomic: remove + recreate).
- [x] Memory cap: refuse to decode if estimated decoded size > configured limit (default 512 MB).
- [x] Integration test: load a real test image, run a full `compute_crop_specs → crop → save_temp` pipeline, verify output dimensions and existence.

### 1.5 `backends/` — KDE backend
- [x] `WallpaperBackend` trait per SPEC §10.1.
- [x] `KdeBackend::availability()` checks `$KDE_FULL_SESSION` / `$XDG_CURRENT_DESKTOP` and returns `Availability::Available` or a specific `WrongEnvironment { reason }`.
- [x] `KdeBackend::apply()` uses `zbus` to call `org.kde.PlasmaShell.evaluateScript` setting per-monitor `Image` plugin source.
- [x] JS payload generated from a versioned template; image paths injected as JSON-quoted literals (never string-concatenated).
- [x] Subprocess/D-Bus rules per SPEC §10.3 (timeout, error capture).
- [x] Manual smoke test on KDE: does the wallpaper actually change? Does it survive a logout/login? Is it bezel-correct?
- [x] Test against a `MockBackend` is written *first*, then `KdeBackend` is implemented to satisfy the same trait — keeps the trait honest.

### 1.6 `config.rs` — config & profiles
- [x] `Config`, `Profile`, `BackendKind`, `MonitorConfig` types with `serde` derives.
- [x] `[[monitor]]` block per SPEC §14.1: `stable_id?`, `name?`, `physical_mm: [u32; 2]`. At least one of `stable_id`/`name` required. Round-trip stable.
- [x] `superpanels monitor configure <NAME-OR-ID> --diagonal 27in --aspect 16:9` and `--mm 597x336` CLI subcommands so users can write `[[monitor]]` blocks without hand-editing TOML.
- [x] Load from `$XDG_CONFIG_HOME/superpanels/config.toml`, write a default if missing (with comments via `toml_edit`).
- [x] Validation pass: returns `ConfigError { path, field, message }`.
- [x] Round-trip test: load → modify → save → reload → identical.
- [x] No I/O in tests; use `tempfile`.

### 1.7 CLI wiring (`superpanels-cli/src/main.rs`)
- [x] `clap::Parser`-derived command tree.
- [x] `superpanels set <IMAGE> [...flags]` per SPEC §11.1, except `--save-as` (Phase 2).
- [x] `superpanels detect`.
- [x] `superpanels config` — print resolved config (debug aid).
- [x] `--dry-run`: print computed crop specs as JSON, skip file write and backend call.
- [x] `--verbose` / `-v` increases tracing level; `--quiet` suppresses non-error output.
- [x] Exit codes per SPEC §11.6.
- [x] `--no-daemon` flag accepted (no-op in Phase 1; daemon arrives in Phase 2).

### 1.8 Documentation (Phase 1 slice)
- [x] README: install (cargo), one-line example, link to SPEC.
- [x] `docs/architecture.md`: a one-page summary of the layout from SPEC §5 with current crate graph.

**Risks for this phase.**
- `kscreen-doctor` output format may be looser than expected (locale dependence; LC_ALL=C needed). Mitigated by capturing fixtures from real systems early.
- `zbus` API churn between minor versions. Pin to a specific minor.
- Image memory bombs. Cap enforced before decode.

---

## Phase 2 — Multi-backend & slideshow

**Goal.** Works on GNOME and basic X11 setups. Other Wayland compositors covered. Folder-driven slideshow runs in the background.

**Definition of done.**
- [ ] Someone on GNOME 46+ can `cargo install superpanels` and have it work without config changes.
- [ ] On Sway with `swww`, the slideshow rotates a folder of panoramas every 30 minutes across both monitors with the bezel respected.
- [ ] `superpanels daemon` starts in the background, drops to systemd user unit if present, and the slideshow survives a daemon restart with state preserved.

### 2.1 Additional backends
- [x] `GnomeBackend`: `gsettings set org.gnome.desktop.background picture-uri[-dark] file://...`. Multi-monitor strategy: composite the per-monitor crops to a single image of the spanning canvas and set that as the wallpaper. Cap composite resolution at 8K long-edge to bound memory; downscale if the canvas exceeds that.
- [x] `HyprlandBackend`: `hyprctl hyprpaper preload` + `hyprctl hyprpaper wallpaper "MONITOR,PATH"` per monitor. If `hyprpaper` isn't running, return a clear error with the start command in the message.
- [x] `SwayBackend`: detect `swww` first, fall back to `swaybg`. `swww img --outputs MON path` for per-monitor.
- [x] `FehBackend`: `feh --bg-fill IMAGE1 IMAGE2 ...`. X11 only.
- [x] `CustomBackend`: `{image_N}` and `{monitor_N}` substitution in a configured template, executed with the standard subprocess rules.
- [x] Auto-detection ordering per SPEC §10.2; pinning via config.

### 2.2 Additional detectors
- [x] `WlrRandrDetector` parsing `wlr-randr --json`.
- [x] `HyprctlDetector` parsing `hyprctl monitors -j`.
- [x] `XrandrDetector` parsing `xrandr --verbose` for X11. Physical-mm extraction.

### 2.3 `slideshow.rs`
- [x] `SlideshowConfig` per SPEC §9.2.
- [x] Picker: respects `sort`, `recent_history_size`, `ImageFilters`.
- [x] State persisted to `$XDG_STATE_HOME/superpanels/state.json` (history, current index, paused).
- [x] Skip-on-unavailable handling.
- [x] Tests: a fake clock + a fake folder source proves rotation, history suppression, and resume-after-restart.

### 2.4 Folder source & `library.rs` (lite)
- [x] Folder scanning (recursive flag) producing `Vec<LibraryEntry>` with resolution, aspect ratio, mtime.
- [x] `notify`-backed FS watch on configured roots; incremental index updates.
- [x] Rayon-parallel initial scan with a progress callback (for the GUI to consume later).
- [x] No SQLite yet — flat in-memory index serialised to its own file at `$XDG_STATE_HOME/superpanels/library-index.json`. **Not** mixed into `state.json` (different write cadence, different migration concerns; the library index is rebuildable from disk while `state.json` is not). SQLite replaces this in Phase 4b with tags.

### 2.5 `daemon/` binary
- [x] `superpanels daemon [--foreground]`.
- [x] Single-instance lock at `$XDG_RUNTIME_DIR/superpanels/daemon.sock`.
- [x] IPC server (length-prefixed JSON) handling: `apply_profile`, `slideshow_next`, `slideshow_prev`, `slideshow_pause`, `redetect`, `current_state`.
- [x] Tokio runtime; slideshow timer via `tokio::time::interval`; cancel-safe.
- [x] FS watcher hooked to library updates.
- [x] Logout-friendly: traps `SIGTERM`, persists state, exits cleanly.
- [x] Optional systemd user unit file generated on demand: `superpanels daemon --install-unit`.

### 2.6 CLI ↔ daemon
- [ ] `superpanels set` etc. detect a running daemon and forward via IPC.
- [ ] `--no-daemon` runs in-process unconditionally (useful for SSH / scripting).
- [ ] `superpanels next` / `prev` / `pause` / `resume` IPC-only commands (require a daemon, friendly error otherwise).
- [ ] `superpanels profile apply` / `list` / `delete` / `rename` / `export` / `import`.

### 2.7 Schedules
- [ ] `Schedule::Daily { at, profile }` (timezone: system local).
- [ ] `Schedule::Sunset { offset, profile }` — uses a small algorithmic sunrise/sunset crate; lat/long configured manually (no IP geolocation).
- [ ] `Schedule::Cron(expr)` — `cron`-crate parsing.
- [ ] Daemon evaluates schedules on a 60-second tick.
- [ ] Tests: a fake clock + a fake config drives every schedule kind.

### 2.8 Save-as profile
- [ ] `superpanels set --save-as NAME [...args]` writes a profile and applies it.

**Risks for this phase.**
- Hyprland's `hyprctl` JSON shape changes between minor versions. Track latest stable; capture multi-version fixtures.
- GNOME's "one image stretched across the desktop" behaviour can interact oddly with HiDPI; the composite-then-scale path needs visual verification.
- IPC socket permission handling on non-`XDG_RUNTIME_DIR` systems. Fall back to `/tmp/superpanels-$UID/daemon.sock` with `0700` mode.

---

## Phase 3 — Tauri shell & tray

**Goal.** `superpanels gui` opens a window. Tray icon works. Profile switching from the tray is functional. UI is functional but not yet polished.

**Definition of done.**
- [ ] `superpanels gui` opens a window with a working monitor canvas (rectangles only — no image yet) and a profile list.
- [ ] System tray shows the icon with a working profile-switch menu.
- [ ] All Tauri IPC commands from SPEC §12.4 are wired through to the daemon's IPC.
- [ ] Closing the GUI window does not stop the daemon. Quit from tray menu stops both.
- [ ] Auto-start opt-in writes `~/.config/autostart/superpanels.desktop`.

### 3.1 Tauri scaffold
- [ ] `superpanels-gui` crate; `tauri.conf.json` configured for Linux build.
- [ ] CSP locked down to local resources only.
- [ ] Allowlist of Tauri APIs: only what we actually use (filesystem in library roots, IPC, tray, dialog/open).
- [ ] Single-window app; window state (size, position) persists to state.json.
- [ ] Build wired into the workspace; `cargo run -p superpanels-gui` opens the window.

### 3.2 Tauri command bindings
- [ ] Every command in SPEC §12.4 declared with `#[tauri::command]`.
- [ ] Each command is a 3-line wrapper: parse args → call core → return `Result`.
- [ ] Commands try the daemon first; fall back to in-process if no daemon.
- [ ] `serde_json` types shared between Rust and Svelte via `ts-rs` (compile-time TS type generation).

### 3.3 Svelte 5 frontend skeleton
- [ ] Vite + Svelte 5 + TypeScript; SvelteKit not used (overkill).
- [ ] Tailwind CSS + a small custom `theme.css` for tokens.
- [ ] Stores: `profile.ts`, `monitors.ts`, `library.ts`, `toast.ts`.
- [ ] Layout: left rail (Profiles / Library / Settings tabs), main area.

### 3.4 Monitor canvas — basic
- [ ] Canvas component renders rectangles to scale based on `detect_monitors`.
- [ ] No image yet — just outlines and bezel bars on a flat background. Proves the geometry pipeline.
- [ ] Resize-aware: re-renders on window resize.

### 3.5 Profile list
- [ ] Reads from `list_profiles`.
- [ ] Click-to-apply via `apply_profile`.
- [ ] Active profile indicator (re-fetched from `current_state`).

### 3.6 System tray
- [ ] SVG icon (light/dark variants).
- [ ] Left click: show/hide window.
- [ ] Right click menu: profile list with active tick, separator, Next/Prev/Pause, Settings, Quit.
- [ ] Tray menu state synchronised to daemon state via a periodic 1-second poll (Tauri tray APIs don't observe; polling is fine and cheap).
- [ ] Tooltip showing active profile + current filename.

### 3.7 Autostart
- [ ] Settings toggle "Start at login" writes/removes `~/.config/autostart/superpanels.desktop`.
- [ ] First-run modal asks; choice persisted.

### 3.8 Notifications
- [ ] `notify-rust`-backed; off by default, opt-in via settings.
- [ ] Surfaces apply errors regardless of opt-in (errors-only mode).

**Risks for this phase.**
- Tauri tray support on Wayland is uneven. KDE has good tray support; GNOME requires AppIndicator extensions; Sway needs `waybar` or similar with StatusNotifierItem support. Document this clearly; the GUI degrades gracefully if no tray host is present (no icon, but window still works).
- WebKitGTK on KDE may not pick up the right theme. Provide `--gtk-theme-from-kde` env var or honour `kdeglobals`.

---

## Phase 4a — Canvas interaction

**Goal.** The headline canvas works. Drag-to-offset, live bezel sliders, accurate physical-mm rendering. No library work yet — the canvas is the unit Phase 4 was over-stuffed for, and it deserves its own phase.

**Definition of done.**
- [ ] Drag the image in the canvas; the crop updates at ≥ 60 fps; releasing applies the new offset to the active profile.
- [ ] Bezel sliders update the canvas in real time.
- [ ] Drag-and-drop image into the window adds it to the active profile.
- [ ] Three clean screenshots: empty state, single-monitor canvas, three-monitor canvas.

### 4a.1 Canvas — rendering pipeline
- [ ] Five-layer compositing per SPEC §12.3:
  - [ ] Wallpaper image layer (using a thumbnail of the source, never the full image during interaction).
  - [ ] Dark overlay.
  - [ ] `destination-out` cut-outs for monitors.
  - [ ] Bezel bars.
  - [ ] Outlines + labels.
- [ ] Renders at canvas resolution, redraws via `requestAnimationFrame`.
- [ ] Monitor labels with name, resolution, and physical size.

### 4a.2 Canvas — accuracy
- [ ] Monitors at correct relative *physical* sizes (mm).
- [ ] Correct relative positions (a monitor lower in `position.y` renders lower).
- [ ] Portrait monitors as rotated rectangles.
- [ ] Bezel bars proportional to mm gap.
- [ ] Visual regression tested by capturing the canvas state to JSON (positions/sizes); diff is meaningful and reviewable.

### 4a.3 Canvas — interactivity
- [ ] Drag offset: pointer events → IPC `preview_crop` → redraw.
- [ ] Bezel sliders: live update; crops + bar widths update on every `input` event.
- [ ] Hover monitor: glow + tooltip with src pixel range.
- [ ] Click monitor: side popout with the exact crop preview (uses the thumbnail to show the slice).
- [ ] `R` key resets offset.
- [ ] Wheel/pinch: zoom 0.5×–2.0× for inspection (does not affect applied result).
- [ ] Apply animation < 400 ms, fade overlay → per-monitor flash → fade in. Replaced by instant transition under `prefers-reduced-motion`.

### 4a.4 Profile editor (canvas-adjacent)
- [ ] Inline form on the right side: image source picker, body type (Span / PerMonitor), fit, bezels, slideshow config.
- [ ] Per-monitor pin UI for `PerMonitor` body (drop image onto monitor in canvas).
- [ ] Schedule editor: visual chooser for daily-time / sunset-offset / cron.
- [ ] Save button or autosave (autosave for non-destructive fields like fit; explicit save for destructive ones like image source).

**Risks for this phase.**
- The canvas's drag interaction sending an IPC roundtrip per frame may bottleneck on Tauri serialisation. If profiling shows > 5 ms per call, port the crop math to TypeScript so it runs in-process and call IPC only on release.

---

## Phase 4b — Library + SQLite

**Goal.** Library grid with thumbnails, tags, favourites. SQLite replaces the Phase-2 JSON index. Drag images *from* the grid *onto* canvas monitors.

**Definition of done.**
- [ ] Library grid renders 1,000 thumbnails smoothly; filtering by tag and aspect ratio works.
- [ ] Tags and favourites persist across daemon restarts via SQLite.
- [ ] Migration from the Phase-2 `library-index.json` runs once on first launch and removes the old file.
- [ ] One clean screenshot of the library grid.

### 4b.1 Library grid
- [ ] `LibraryGrid` virtualised list (e.g. `svelte-virtual-list`); renders only visible rows.
- [ ] `library_thumbnail` IPC returns WebP bytes; cached client-side via `URL.createObjectURL`.
- [ ] Filters: tag chips, aspect ratio dropdown, min-resolution input.
- [ ] Sort: Date added, Date modified, Resolution, Last shown.
- [ ] Right-click context menu: Apply now, Set for monitor…, Tag…, Favourite, Reveal in file manager, Delete from library.
- [ ] Search box filtering on filename + tag.
- [ ] Drag-and-drop image *out* of the grid onto a monitor in the canvas → assigns image to that monitor (PerMonitor body).

### 4b.2 Library backing — SQLite
- [ ] Replace the Phase-2 in-memory + `library-index.json` index with SQLite per SPEC §14.5.
- [ ] One-shot migration from `library-index.json` on first run of the new version; removes the old file once committed.
- [ ] Schema migrations via `PRAGMA user_version`.
- [ ] Tag operations idempotent; case-insensitive matching.

**Risks for this phase.**
- Thumbnail generation for a large library is the GUI's first-impression cost. Move to a background queue with visible progress, never block the grid.

---

## Phase 4c — Polish & accessibility

**Goal.** The first version that's *pleasant* to use. Onboarding, settings, theming, accessibility audit.

**Definition of done.**
- [ ] `prefers-reduced-motion` respected throughout.
- [ ] Five clean screenshots: empty state, single-monitor canvas, three-monitor canvas, library grid, settings.
- [ ] Keyboard-only walk-through of every screen succeeds.
- [ ] Orca screen-reader smoke test passes for the main flows.

### 4c.1 Settings panel
- [ ] General: theme, autostart, notifications, default profile.
- [ ] Library: roots add/remove, recursive toggle, thumbnail size.
- [ ] Backend: prefer dropdown, custom command field with safety callout.
- [ ] Advanced: log level, memory cap, debug pane (raw IPC responses for support).

### 4c.2 Polish pass
- [ ] Tailwind theme tokens for a consistent dark palette.
- [ ] Toasts: bottom-right, dismiss on click, auto-dismiss after 5 s, errors persist until dismissed.
- [ ] Empty states: canvas shows a friendly placeholder with onboarding hint; library shows "no images — add a folder" CTA.
- [ ] Keyboard shortcuts wired per SPEC §12.5; a `?` overlay shows the full list.
- [ ] Loading indicators on long ops (initial library scan, large image apply).
- [ ] Focus outlines preserved (no `outline: none`).

### 4c.3 Accessibility
- [ ] Every interactive control has an `aria-label` or visible label.
- [ ] Tab order audit: keyboard-only walk-through of every screen.
- [ ] Colour-contrast check: text ≥ 4.5:1 against background (WCAG AA).
- [ ] Respect `prefers-reduced-motion`.
- [ ] Screen reader smoke test with Orca on the dev machine.

---

## Phase 5 — Packaging & first release (v0.7)

**Goal.** Anyone on Arch can install Superpanels in one command. The release artefacts are pre-built and signed. The README sells the project in 30 seconds.

**Definition of done.**
- [ ] `yay -S superpanels` and `yay -S superpanels-gui` work on Arch and CachyOS.
- [ ] `cargo install superpanels` works on a fresh Rust toolchain.
- [ ] GitHub release tagged `v0.7.0` (first public, GUI-bundled release) with binary attachments.
- [ ] README has one screenshot, an install line for Arch, an install line for crates.io, and a paragraph explaining what's special.

### 5.1 PKGBUILDs (AUR)
- [ ] `superpanels` (CLI-only): `makedepends=(rust)`, `depends=()`.
- [ ] `superpanels-gui`: `makedepends=(rust nodejs npm)`, `depends=(webkit2gtk-4.1)`.
- [ ] PKGBUILDs reviewed against AUR style guidelines.
- [ ] `.SRCINFO` generated.
- [ ] Submitted to AUR.

### 5.2 Crates.io
- [ ] `cargo publish` for `superpanels-core` then `superpanels`.
- [ ] Crate metadata complete: description, keywords, categories, repository, documentation URL.
- [ ] `[package.metadata.docs.rs]` configured for `cargo doc` builds.

### 5.3 Flatpak (best-effort)
- [ ] Flatpak manifest under `packaging/flatpak/io.github.alex.superpanels.yaml`.
- [ ] Builds locally; submission to Flathub deferred to v0.8.

### 5.4 GitHub Actions release pipeline
- [ ] On tag push: build CLI binary (statically linked where viable), build GUI binary (Tauri bundler), attach to release.
- [ ] Build matrix: x86_64, aarch64.
- [ ] SHA-256 checksums alongside binaries.
- [ ] Optional `cosign` signing.

### 5.5 Documentation
- [ ] `README.md`: hero screenshot, install lines, three-line pitch, link to SPEC and the docs site.
- [ ] `docs/getting-started.md`.
- [ ] `docs/cli-reference.md` (auto-generated from clap where possible).
- [ ] `docs/configuration.md` (the TOML schema).
- [ ] `docs/troubleshooting.md`.
- [ ] `CHANGELOG.md` following Keep-a-Changelog.

**Risks for this phase.**
- Static linking against glibc on Arch is harder than it looks. Plan B: dynamic glibc linkage with `glibc >= 2.17` documented; users on minimal containers get a warning.
- AUR review feedback may require PKGBUILD changes; budget a day.

---

## Phase 6 — Stabilisation toward 1.0

**Goal.** Schema-frozen, perf-budget-enforced, accessibility-audited, ready for a 1.0 declaration.

**Definition of done.**
- [ ] No breaking config changes since v0.7; migration code present for older configs.
- [ ] Performance budgets from SPEC §19 enforced by CI benchmarks (regression > 10% fails the build).
- [ ] Localisation pipeline operational; English catalogue complete.
- [ ] Accessibility audit complete (Orca + axe-core for the web view); known issues triaged.
- [ ] One external review (a Linux-power-user friend) used the app for a week and reported no blocking bugs.

### 6.1 Schema freeze
- [ ] Audit `Config`, `Profile`, `state.json`, `library.db` schemas; make any pending changes.
- [ ] Tag the schema as `v1`. Future changes require migrations, not breakage.

### 6.2 Performance enforcement
- [ ] `criterion` benches for: bezel math (10k random layouts), library scan (5k images), single-image apply (4K, 8K).
- [ ] `cargo bench` runs nightly in CI; results compared to `main`; > 10% regression fails.

### 6.3 Localisation
- [ ] All UI strings in `ui/locales/en.ftl`.
- [ ] All CLI human-readable messages in `crates/superpanels-cli/locales/en.ftl`.
- [ ] `t!("…")` macro everywhere; no string literals in UI files.
- [ ] CONTRIBUTING.md has a "Adding a translation" section.

### 6.4 Documentation site
- [ ] `mdbook` site under `docs/` deployed to GitHub Pages on push to `main`.
- [ ] Tutorials: "Spanning a panorama", "Per-monitor profiles", "Slideshow + schedule", "Custom backend".
- [ ] Architecture deep-dive (the bezel math walkthrough) — turn SPEC §4 into prose with diagrams.

### 6.5 1.0 release
- [ ] Tag `v1.0.0`.
- [ ] Blog post / project announcement.
- [ ] Submit to `r/unixporn`, `r/archlinux`, HN, with the showcase screenshot.

---

## Cross-cutting concerns

These run through every phase and don't belong to any single one.

### Testing
- Unit tests in every module; coverage tracked but not gated on a threshold.
- Snapshot tests for parsers via `insta`.
- Property tests for layout via `proptest`.
- Integration tests using `MockBackend`.
- Golden-image tests for the image pipeline (post-Phase 1).
- Manual smoke checklist (`docs/release-checklist.md`) updated each phase, run before each release.

### CI / quality
- `cargo test --workspace --all-features` on every PR.
- `cargo clippy --all-features -- -D warnings`.
- `cargo fmt --check`.
- `cargo audit` weekly.
- `cargo deny check` for licence compliance.
- `cargo machete` to catch unused dependencies.
- `npm audit` weekly on the UI.

### Logging
- `tracing` from day one. Every subprocess call, every IPC request, every config load logs at `info`/`debug`.
- Structured fields, never `format!` into the message.

### Documentation hygiene
- `cargo doc` builds clean (no missing docs warnings on public items in `superpanels-core`).
- Top-level types in the public API have a one-paragraph rustdoc with an example.
- SPEC and PLAN are kept current; "spec drift" is a PR-blocking review comment.

### Security review at each phase exit
- New subprocess spawns: re-confirm the rules in SPEC §10.3.
- New file paths read/written: re-confirm scope.
- New IPC commands: re-confirm input validation.

### Performance baselines
- Phase 1: bench `compute_crop_specs` for 1, 3, 6, 9 monitors. Capture baseline.
- Phase 2: bench library scan for 100, 1k, 10k images. Capture baseline.
- Phase 4: bench canvas redraw frame time. Capture baseline.

---

## Risk register

| ID | Risk | Likelihood | Impact | Mitigation | Phase |
|---|---|---|---|---|---|
| R1 | KDE D-Bus call fails or is rate-limited | Med | High | Backoff + retry; `evaluateScript` has been stable for years; capture stderr | 1 |
| R2 | ~~EDID-derived physical mm missing on some monitors~~ — *resolved during Phase 0 spike*: kscreen-doctor doesn't expose physical mm at all, so the design now sources it from per-monitor config (§14.1). What was a risk became a deliberate design decision. | — | — | n/a | — |
| R3 | Hyprland JSON shape changes between minor versions | Med | Med | Pin lower bound; capture multiple-version fixtures; quick parser updates | 2 |
| R4 | GNOME composite memory spike on huge canvases | Med | Med | Cap at 8K long-edge; downscale below that; document in troubleshooting | 2 |
| R5 | Tauri tray UX differences across compositors | High | Med | Detect `StatusNotifierItem` host; degrade gracefully (no tray, window still works); document | 3 |
| R6 | IPC roundtrip in canvas drag too slow | Low | Med | Profile early; if > 5 ms/frame, port crop math to TS for live preview, IPC on release | 4 |
| R7 | Thumbnail cache grows unbounded | Med | Low | Bounded cache (500 MB default), LRU eviction | 4 |
| R8 | AUR review rejection | Low | Low | Follow style guide; budget revision time | 5 |
| R9 | Static linking on glibc systems is brittle | Med | Low | Document glibc requirement; offer dynamic build; provide Flatpak | 5 |

---

## Decisions to make early

These need answers before the relevant phase begins. Tracked as GitHub issues once the repo is public.

1. **Licence.** MIT, Apache-2.0/MIT dual, or GPL-3.0? *Recommendation: dual MIT/Apache-2.0 (Rust ecosystem default).*
2. **MSRV (Minimum Supported Rust Version).** *Recommendation: latest stable at start of work; bump freely until v1.0; document in `rust-toolchain.toml`.*
3. **Tauri vs Iced vs egui.** *Recommendation: stick with Tauri v2 (canvas work easier in a web view, Svelte ecosystem). Revisit if WebKitGTK becomes a packaging issue.*
4. **Crate naming.** Flat `superpanels-{core,cli,gui,daemon}` or single binary with subcommands? *Recommendation: workspace of crates internally; single `superpanels` binary externally with subcommand dispatch (best of both — clean architecture, simple packaging).*
5. **EDID hash form.** Full EDID vs (manufacturer + model + serial). *Recommendation: hash of (manufacturer + model + serial). Cable swap shouldn't break per-monitor profile data.*
6. **Thumbnail format.** WebP, AVIF, or PNG. *Recommendation: WebP (decoded by browsers natively, small, decent quality at 80%; AVIF encode is too slow per-image).*
7. **State storage.** SQLite from day one or migrate later? *Recommendation: JSON in Phase 2, SQLite in Phase 4 when tags arrive. Migration code is a known cost we accept.*

---

## First commits playbook

A suggested sequence for the first day's work, optimised for getting a green CI as early as possible.

1. **Commit 1 — workspace scaffold.** `Cargo.toml` workspace + `crates/superpanels-core` lib + `crates/superpanels-cli` bin + a `hello` integration test. CI green.
2. **Commit 2 — `Monitor` & `Rotation` types.** With serde derives and a round-trip JSON test. Sets the data-model foundation everything else hangs from.
3. **Commit 3 — `BezelConfig`, `CropSpec`, `Rect`, `FitMode`.** Pure data types, no logic.
4. **Commit 4 — bezel math: trivial cases.** Single monitor + two identical monitors with zero gap. Tests pin the obvious behaviour first.
5. **Commit 5 — bezel math: uniform gap, mixed PPI.** The interesting cases. Property tests added.
6. **Commit 6 — bezel math: rotation + 2×2 grid.** Closes the matrix.
7. **Commit 7 — KSscreen-doctor parser.** Tested against captured fixtures only; no live system.
8. **Commit 8 — manual override parser.** `--monitors` syntax tested with multiple shapes.
9. **Commit 9 — `superpanels detect` CLI.** First end-to-end CLI command. Ship-able.
10. **Commit 10 — `image.rs` load/scale/crop/save_temp.** Pure-Rust pipeline, integration test against a real test image.

By commit 10, you have a passing CI, a working `detect` command, a tested bezel pipeline, and a tested image pipeline. Phase 1.5 (the KDE backend) is the next 2–3 commits and the first one whose tests can't run on CI without a live KDE session — handle it via the `MockBackend`-first approach so the trait is well-defined before the real backend is written.

---

## Where to start, in one line

`crates/superpanels-core/src/display/kscreen.rs` against a captured fixture, with the test written first. Everything else hangs off knowing what monitors exist; everything purely arithmetic (`layout.rs`) can develop in parallel from the same `Monitor` type.
