# Superpanels — Technical Specification

> A lightweight, multi-monitor-aware wallpaper manager for Linux.
> Bezel-correct image spanning. Folder-driven slideshows. Slick GUI + tray.
> Single static binary. Zero runtime dependencies (CLI). Pure-Rust core.

---

## Table of contents

1. [Goals & non-goals](#1-goals--non-goals)
2. [Personas & user stories](#2-personas--user-stories)
3. [Core concepts](#3-core-concepts)
4. [The bezel & layout math](#4-the-bezel--layout-math)
5. [Application architecture](#5-application-architecture)
6. [Display detection](#6-display-detection)
7. [Wallpaper sources & library](#7-wallpaper-sources--library)
8. [Image processing & colour](#8-image-processing--colour)
9. [Profiles, schedules & slideshows](#9-profiles-schedules--slideshows)
10. [Backend system](#10-backend-system)
11. [CLI interface](#11-cli-interface)
12. [GUI — Tauri + Svelte](#12-gui--tauri--svelte)
13. [System tray](#13-system-tray)
14. [Configuration & state](#14-configuration--state)
15. [Logging & observability](#15-logging--observability)
16. [Error handling philosophy](#16-error-handling-philosophy)
17. [Security & sandbox considerations](#17-security--sandbox-considerations)
18. [Accessibility & i18n](#18-accessibility--i18n)
19. [Performance targets](#19-performance-targets)
20. [Testing strategy](#20-testing-strategy)
21. [Packaging & distribution](#21-packaging--distribution)
22. [Out of scope (v1)](#22-out-of-scope-v1)
23. [Open questions](#23-open-questions)

---

## 1. Goals & non-goals

### 1.1 Primary goals
- **Bezel-aware spanning.** Take a wide or panoramic image and span it across multiple monitors so the image content remains continuous *as the eye sees it across the desk* — bezels included. The image accounts for the physical gap between screens, so content isn't shifted, squashed, or duplicated.
- **Multi-monitor first.** The interesting case is two-or-more displays of mixed size, mixed PPI, and mixed orientation. Single-monitor is supported but is not the design centre.
- **Folder-driven slideshows.** Point at a folder of wallpapers and have the app rotate through them on a schedule, with sane filtering and history so the same image doesn't appear twice in a row.
- **Lightweight & static.** Trivially installable. `pacman -S superpanels` or `cargo install superpanels`. No Python, no virtualenvs, no system library drift.
- **Slick optional GUI.** Tauri + Svelte 5. The headline UI element is a live, scaled, bezel-accurate monitor-preview canvas that lets you compose the wallpaper before applying.
- **CLI-first.** Every feature reachable from the GUI is reachable from the CLI for scripting and automation. The GUI calls the same library the CLI does.
- **Extensible backends.** Each desktop environment is an isolated, testable module behind a single trait. Adding a backend should not require touching unrelated code.

### 1.2 Quality goals
- **Looks great.** Polish is a feature, not garnish. Default theme is dark, KDE-Breeze-adjacent, with smooth canvas updates.
- **Fast.** Apply a wallpaper in under 500 ms on a typical setup (excluding the time the compositor takes to redraw). GUI canvas redraws stay above 60 fps during drag interactions on a Ryzen 5600 / integrated graphics.
- **Predictable.** A given config + image + monitor layout always produces the same result. No hidden state.
- **Recoverable.** Bad config or backend failure never leaves the desktop in a broken state; it returns an error and the previous wallpaper remains.

### 1.3 Non-goals
- Cross-platform (Windows/macOS) — not in v1; the backend trait is shaped to allow it later.
- Live wallpaper / video / shader wallpapers.
- Online wallpaper sources (wallhaven, unsplash) — possibly post-v1, not core.
- Per-monitor colour calibration / ICC profile management.
- Perspective correction (Superpaper-style angled-monitor warping).
- Wallpaper editing (cropping, colour adjustment) beyond what's needed to fit the canvas.

---

## 2. Personas & user stories

### 2.1 Personas

**The triple-monitor power user (primary).** Three screens: a 34" ultrawide flanked by two 27" 4Ks, one rotated portrait. Wants a single panorama to span the whole desk, with bezel correction, and a folder of curated panoramas to rotate through every couple of hours.

**The KDE tinkerer.** Comfortable in the terminal, but reaches for a GUI when designing things visually. Wants the canvas to *look* like the desk so they can compose without applying first.

**The minimalist Sway/Hyprland user.** Lives in the CLI. Will never open the GUI. Needs scripting hooks, a `--dry-run`, and predictable JSON output for `detect`.

### 2.2 Headline user stories

1. *"Drop a 7680×2160 panorama into Superpanels and have it span my three monitors with the bezel gap accounted for."* — see §4, §7, §10.
2. *"Point Superpanels at `~/Pictures/walls/panoramas/` and have it rotate through them every 30 minutes, never repeating the last 10."* — see §9.
3. *"Open the GUI, drag the image around the canvas to choose what bit lands on which monitor, then click Apply."* — see §12.
4. *"Have a 'work' profile (calm panoramas) and a 'home' profile (game art per monitor) that I can switch between from the tray."* — see §9, §13.
5. *"Run `superpanels set my.jpg` over SSH on a headless gaming PC to set the wallpaper before I sit down."* — see §11.
6. *"Tell Superpanels my monitor is portrait-rotated and have it carve out the right slice of a wide image to land on it correctly."* — see §3, §4.
7. *"The slideshow should pick images that suit my monitor layout — skip the 1024×768 squares and prefer ultrawides for spanning."* — see §7.

---

## 3. Core concepts

### 3.1 Monitor

A physical display as reported by the system, normalised into Superpanels' internal model.

```rust
struct Monitor {
    id: MonitorId,                       // runtime-only identity, assigned at detection time
                                         //   (left-to-right then top-to-bottom). Never persisted.
    name: String,                        // e.g. "DP-1", "HDMI-A-1" (may not be portable across reboots)
    stable_id: Option<String>,           // compositor-supplied stable ID (KDE per-output UUID etc.);
                                         //   used as MonitorRef.stable_id when present
    position: (i32, i32),                // top-left corner in the logical desktop (px, post-scale)
    resolution: (u32, u32),              // pixel dimensions (w, h) in native orientation
    physical_size_mm: Option<(u32, u32)>, // physical dimensions in mm (w, h, native orientation);
                                         //   sourced from per-monitor config (§14.1), NOT detection.
                                         //   None until the user has provided one — bezel math
                                         //   refuses to run without it.
    scale: f64,                          // HiDPI scale factor (1.0, 1.25, 1.5, 2.0, ...)
    rotation: Rotation,                  // None | Left (90 CCW) | Right (90 CW) | Inverted (180)
    refresh_hz: Option<f32>,             // for display in detect output; not used in math
    primary: bool,
    ppi: Option<f64>,                    // derived: pixels per inch, post-rotation; None when
                                         //   physical_size_mm is None.
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MonitorId(u32);

enum Rotation { None, Left, Right, Inverted }
```

`MonitorId` is **runtime-only**: it's assigned during detection, used to address monitors during a single apply, and never persisted. For data that must survive reboots and dock re-plugs (per-monitor profile assignments, bezel overrides, physical-size config) use [`MonitorRef`](#64-monitor-identity-across-reboots) instead.

`physical_size_mm` is **user-supplied**, not detected. The reason: `kscreen-doctor` (and most compositor CLIs) do not expose physical mm; reading EDID from sysfs is a workable fallback but EDID values are sometimes wrong and irrelevant on virtual displays. Since the user is already configuring bezel widths in mm, asking them for monitor dimensions in mm is a small additional step (and the GUI's first-run flow can prefill from "diagonal + aspect" — see §12.6). When `physical_size_mm` is `None`, bezel math returns `LayoutError::PhysicalSizeMissing` listing the monitors that need configuration.

`rotation` matters for the layout math: a portrait monitor's *physical* width is its (rotated) short side and its height is its long side. Everything in `physical_size_mm` is recorded in native orientation; the layout module applies the rotation when building the desktop's physical canvas.

### 3.2 BezelConfig

Physical gap sizes between adjacent screens, specified in millimetres.

```rust
struct BezelConfig {
    horizontal_mm: f32,                                // uniform gap between any pair of horizontally adjacent monitors
    vertical_mm: f32,                                  // uniform gap between any pair of vertically adjacent monitors
    overrides: HashMap<(MonitorRef, MonitorRef), f32>, // optional per-pair override; key is sorted-pair
}
```

Default is uniform `horizontal_mm` / `vertical_mm`, which covers ~90% of real setups. Overrides exist for the rare case where bezel widths differ — e.g., a thin-bezel ultrawide between two thick-bezel old IPS panels. The override key is normalised so `(a, b)` and `(b, a)` collapse to the same entry; see [`MonitorRef`](#64-monitor-identity-across-reboots) for why we key on `MonitorRef`, not `MonitorId`.

### 3.3 CropSpec

The rectangle within the source image that maps to a given monitor after bezel compensation, plus the per-monitor render parameters.

```rust
struct CropSpec {
    monitor_id: MonitorId,
    src_rect: Rect,           // (x, y, w, h) in source-image pixels
    dst_size: (u32, u32),     // target monitor pixel dimensions (post-rotation)
    rotation: Rotation,       // applied during render so the file lands right-side-up
    fit: FitMode,             // how `src_rect` was chosen (informational; useful for the GUI)
}
```

### 3.4 Profile

A named, persistent configuration that bundles the inputs needed to set a wallpaper. The shape is built around the principle "make illegal states unrepresentable": *how images map to monitors* (span vs per-monitor) and *whether the source rotates over time* (single vs slideshow) are orthogonal concerns expressed by nested enums, so a configuration like `mode=Slideshow, images=Single` is unrepresentable rather than runtime-validated.

```rust
struct Profile {
    name: String,
    body: ProfileBody,
    bezels: BezelConfig,
    backend_override: Option<BackendKind>,
    schedule: Option<Schedule>,           // optional time-of-day trigger (see §9)
}

enum ProfileBody {
    /// One image at a time, spanned across all monitors with bezel correction.
    /// The "source" can be a single file or a rotating set (slideshow).
    Span(SpanProfile),
    /// One image pinned per monitor. No spanning; bezels are irrelevant per crop.
    PerMonitor(PerMonitorProfile),
}

struct SpanProfile {
    source: SpanSource,
    fit: FitMode,
    offset: (i32, i32),                   // image-position offset in canvas px (see §8.3)
}

enum SpanSource {
    Single(PathBuf),
    Slideshow { images: ImageSet, config: SlideshowConfig },
}

struct PerMonitorProfile {
    /// Each monitor gets exactly one image. Order is irrelevant — the layout
    /// step resolves MonitorRef → live MonitorId at apply time.
    assignments: Vec<(MonitorRef, PathBuf)>,
    fit: FitMode,
}

enum FitMode { Fill, Fit, Stretch, Center }
```

### 3.5 ImageSet

The pool of images a slideshow draws from. See §7 for how images are scanned, indexed, and filtered.

```rust
enum ImageSet {
    Folder { path: PathBuf, recursive: bool, filters: ImageFilters },
    Playlist(Vec<PathBuf>),               // hand-curated rotation list
}

struct ImageFilters {
    min_resolution: Option<(u32, u32)>,
    aspect_ratios: Option<AspectFilter>,  // Any | Wide | Standard | Custom(min, max)
    tags: Option<Vec<String>>,            // matches user-applied tags (see §7.3)
    favourites_only: bool,                // shorthand for filtering on the favourite flag
}
```

---

## 4. The bezel & layout math

The principle: **the image maps to the *physical* screen plane, including the space occupied by bezels.** The crops handed to each monitor are non-overlapping; the content that "falls" in the bezel gap simply isn't drawn, but it is *accounted for* in the layout, so visual continuity is preserved across the bezel.

### 4.1 Worked example — two identical monitors, uniform gap

```
Physical layout (mm):   [==monitor 1==][bezel|bezel][==monitor 2==]
                         <--- W1 mm ---> <-- G mm --> <--- W2 mm --->

Total physical width = W1 + G + W2 (mm)
Image pixels per mm  = image_width_px / total_physical_width_mm

Monitor 1 crop:       x = 0,                     w = W1_mm * px_per_mm
Monitor 2 crop:       x = (W1 + G) * px_per_mm,  w = W2_mm * px_per_mm
```

### 4.2 Mixed PPI

When monitors have different pixel densities, normalise to a reference PPI before computing crops, so the image appears at the same physical scale on each screen. Reference PPI is the maximum across all monitors by default; user can override per-profile.

### 4.3 Mixed orientation

A portrait monitor contributes its rotated dimensions to the physical canvas: a 27" 16:9 panel in portrait orientation becomes ~336 mm wide × 597 mm tall. The crop handler rotates the cropped pixel rectangle by the monitor's `rotation` before saving, so the temp file is always written in the orientation the monitor will display it. The compositor sees a normal upright image; rotation is *baked in* during processing.

### 4.4 General algorithm

```
1. Collect monitors, sort by (position.y, position.x).
2. Apply rotation: for each monitor compute effective_width_mm,
   effective_height_mm based on rotation.
3. Group into rows by y-overlap (a row = monitors whose vertical ranges overlap).
4. Pick reference PPI = max(monitor.ppi) or user-configured.
5. For each row, build a 1-D physical layout:
     row_starts[i] = sum of (effective_widths_mm + gap_mm) before i
     row_total_width_mm = sum of effective_widths_mm + gaps_mm
6. Stack rows vertically with vertical gap_mm between rows:
     canvas_height_mm = sum of row_heights + (n_rows - 1) * vertical_gap
7. Convert the canvas to reference-PPI pixels:
     canvas_w_px = canvas_width_mm * ref_ppi / 25.4
     canvas_h_px = canvas_height_mm * ref_ppi / 25.4
8. Scale the source image to fit canvas (per FitMode).
9. For each monitor, compute src_rect in source-image pixels:
     - origin_mm = (row_start[col], col_start[row])
     - size_mm   = (effective_width_mm, effective_height_mm)
     - convert mm → reference PPI px → source-image px (account for image scale)
10. Resample monitor's crop to its native resolution (post-rotation).
11. Apply rotation to the resampled image.
12. Hand each (image_path, monitor_id) pair to the backend.
```

### 4.5 Edge cases the layout module must handle

- A single monitor (degenerate canvas — bezel math is a no-op).
- 3+ monitors in a single row.
- 2×2 grid (Sway/Hyprland tiling-WM users do this).
- Mixed sizes side-by-side (e.g. 34" ultrawide + 24" 1080p).
- One landscape + one portrait (the headline rotation case).
- Monitors with non-zero `position` offset that the desktop doesn't expose as a row (e.g. one monitor 200 px lower than the other).
- HiDPI + scale factor (a 2.0-scale 4K reports 1920×1080 logical px but 3840×2160 native).

### 4.6 What the math deliberately does *not* do

- It doesn't try to hide image content "behind" the bezel by *omitting* the bezel pixels from the source — that produces visible duplication at the seam. It crops at the bezel boundary and skips the gap.
- It doesn't perspective-correct angled monitors. If the user toes their monitors in, that's their problem (see roadmap).
- It doesn't try to be smart about subject framing (e.g. "keep the face on the centre monitor"). v2.

---

## 5. Application architecture

### 5.1 Process model

Superpanels ships as a single binary with multiple personalities, selected by subcommand:

| Personality | Invocation | What it does |
|---|---|---|
| One-shot CLI | `superpanels set …` | Apply a wallpaper, exit. |
| Detector | `superpanels detect [--json]` | Print monitor layout, exit. |
| Profile actions | `superpanels profile …` | List/apply/edit/delete profiles, exit. |
| Daemon | `superpanels daemon` | Background process: slideshow timer, schedule triggers, FS watch, IPC server. No UI. |
| GUI | `superpanels gui` | Tauri window + system tray. Spawns/connects to daemon for background work. |

Single-binary keeps packaging trivial. Each subcommand is dispatched in `main.rs`; the rest is library code.

### 5.2 Single-instance behaviour

- The daemon and GUI are mutually-aware: at most one daemon runs per user session. The lock is a Unix domain socket at `$XDG_RUNTIME_DIR/superpanels/daemon.sock`.
- If the user runs `superpanels gui` and a daemon is already running, the GUI connects to it over the IPC socket. If no daemon is running, the GUI spawns one as a child and supervises it.
- Running `superpanels gui` twice raises the existing window (via the IPC socket) instead of opening a second window.

### 5.3 IPC protocol

Length-prefixed JSON over the Unix socket. Versioned (`{"v": 1, "method": "...", "params": {...}}`). Methods mirror the Tauri commands so the GUI's command handler is a thin pass-through. The CLI also speaks IPC: `superpanels set` running while a daemon is up sends a `set` request to the daemon rather than re-detecting + re-applying itself, so the daemon's state (current image, slideshow position) stays consistent.

If the daemon isn't running, the CLI does the work in-process and exits — no daemon required for one-shot use.

### 5.4 Library / wrapper layout

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

### 5.5 Threading

- The core library is `Send + Sync`-friendly; long-running ops (image processing, FS scan) are on a Tokio runtime in the daemon.
- The Tauri GUI invokes core via `tauri::async_runtime::spawn_blocking` for image work to keep the UI thread free.
- The slideshow timer uses `tokio::time::interval` rather than thread-sleep, so it's cancellation-safe.

---

## 6. Display detection

Detection produces a *layout-only* `Monitor` struct (positions, resolutions, scale, rotation, name, optional `stable_id`). Physical sizes are merged in afterwards from per-monitor config (§14.1). This split exists because no Linux compositor CLI reliably exposes `physical_size_mm` — KDE's `kscreen-doctor -o` doesn't, and EDID-from-sysfs is sometimes wrong on real hardware. Asking the user for monitor mm is one extra config step on top of the bezel mm they're already providing; in exchange we get a uniform detection surface and predictable correctness.

Detection is attempted in priority order, stopping at the first detector that succeeds and returns a non-empty monitor list.

| Priority | Detector | Detection condition | Source of truth |
|---|---|---|---|
| 1 | `kscreen-doctor -o` | KDE session detected | KDE Plasma. Provides per-output UUID usable as `stable_id`. Run with `NO_COLOR=1` so the parser doesn't have to strip ANSI. |
| 2 | `hyprctl monitors -j` | `$HYPRLAND_INSTANCE_SIGNATURE` set | Hyprland JSON output. |
| 3 | `swaymsg -t get_outputs` | `$SWAYSOCK` set | Sway-native; more reliable than `wlr-randr` on Sway specifically. |
| 4 | `wlr-randr --json` | wlroots compositor without `$SWAYSOCK` | Generic wlroots JSON output. |
| 5 | `xrandr --verbose` | `$DISPLAY` set, Wayland not in use | X11 fallback. |
| 6 | Manual override | `--monitors` CLI flag, or config | Always wins if set. |

Each detector runs as a subprocess with a **5-second timeout**. If all fail, return: `Could not detect monitor layout. Try --monitors WxH+X+Y,WxH+X+Y... to specify manually, or run 'superpanels detect --debug' to see what was attempted.`

After detection, layout `Monitor`s are merged with the user's per-monitor config (§14.1) — matching by `stable_id` first, falling back to `name`. The merged `physical_size_mm` is `None` for any monitor not yet in config; the bezel-math entry point (`compute_crop_specs`) returns `LayoutError::PhysicalSizeMissing { monitors: Vec<MonitorRef> }` listing exactly which monitors need configuration. The CLI surfaces this as a friendly "run `superpanels monitor configure DP-1` to provide its dimensions" message; the GUI surfaces it as a first-run modal.

### 6.1 Detector contract

Each detector implements:

```rust
trait DisplayDetector {
    fn name(&self) -> &str;
    fn availability(&self) -> Availability; // env-var or PATH check; never spawn
    fn detect(&self) -> Result<Vec<Monitor>, DetectError>;
}

enum Availability {
    Available,
    ToolMissing { tool: &'static str },     // e.g. "kscreen-doctor not on PATH"
    WrongEnvironment { reason: &'static str }, // e.g. "$KDE_FULL_SESSION not set"
    Disabled,                               // user pinned a different detector
}

#[derive(Debug, thiserror::Error)]
enum DetectError {
    #[error("subprocess `{cmd}` failed: {stderr}")]
    Subprocess { cmd: String, stderr: String },
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout { cmd: String, seconds: u64 },
    #[error("could not parse output of `{cmd}`: {message}")]
    Parse { cmd: String, message: String },
    #[error("detector returned an empty monitor list")]
    EmptyResult,
}
```

`Availability` returns an enum (not a `bool`) so `superpanels detect --debug` can explain *why* each detector was skipped. Errors are typed so the orchestrator can react differently to "tool missing" vs "tool present but failed" vs "parser broken".

Each detector is *individually unit-tested* against captured real-world output samples stored under `crates/superpanels-core/tests/fixtures/display/`. We never hit the system in tests.

### 6.2 Manual override syntax

`--monitors WxH+X+Y[@SCALE][/ROT][?WMMxHMM],...`

- `1920x1080+0+0` — layout-only override; physical mm still expected from `[[monitor]]` config.
- `2560x1440+0+0@1.5/right?597x336` — full override including 597×336 mm physical (skips config merge for this monitor).

Useful for SSH/headless/CI environments and for the test suite.

### 6.3 Live re-detection

The daemon re-detects monitors on:
- A `SIGHUP` signal.
- IPC `redetect` request (e.g. user clicked Refresh in the GUI).
- An optional, opt-in periodic re-detect every 60s for laptop dock hot-plug. Off by default; an FS-watch on `/sys/class/drm` is preferred where it works.

### 6.4 Monitor identity across reboots

`monitor.name` is unstable (a USB-C dock can re-label DP-1 to DP-2). For per-monitor persistent data — custom bezel overrides, per-monitor image assignments, **and physical-mm config** — we key on a stable identifier when one is available, falling back to `name`. Profiles and config refer to monitors by:

```rust
struct MonitorRef {
    stable_id: Option<String>,  // KDE per-output UUID; or hash of EDID manufacturer+model+serial
    name: Option<String>,       // "DP-1"; fallback when stable_id is unavailable
}
```

The layout step resolves `MonitorRef` to a live `MonitorId` (many-to-one: a `MonitorRef` matches the unique live `Monitor` whose `stable_id` matches, else whose `name` matches).

**`stable_id` sources, by detector:**

| Detector | Source of `stable_id` |
|---|---|
| `kscreen-doctor -o` | The per-output UUID printed in the `Output:` line (e.g. `f7f0f124-9e9b-4ef0-91a7-426d58091760`) — KDE generates this deterministically from EDID, so we use it directly without our own hashing. |
| `hyprctl monitors -j` | The `serial` field from JSON output. |
| `swaymsg -t get_outputs` | `make + model + serial` from the JSON output, hashed. |
| `wlr-randr --json` | `make + model + serial` if exposed; else `None`. |
| `xrandr --verbose` | EDID hex-dump under `EDID:` block, parsed for manufacturer/model/serial, hashed. |

When a detector can't supply `stable_id`, the fallback is `name`. This breaks if the user re-plugs a dock and `DP-1` now refers to a different physical monitor — we accept that ambiguity, and the GUI prompts to re-confirm assignments after detecting that the physical configuration has changed.

---

## 7. Wallpaper sources & library

A "wallpaper management app" needs a real notion of a *library*, not just a single file path. The library is the user's set of available images; profiles and slideshows draw from it.

### 7.1 Library model

```rust
struct LibraryEntry {
    path: PathBuf,
    resolution: (u32, u32),     // cached after first scan
    aspect_ratio: f32,
    file_size: u64,
    modified: SystemTime,
    tags: Vec<String>,          // user-applied
    favourite: bool,
    last_shown: Option<SystemTime>,
    show_count: u32,
}
```

The library is a flat index over one or more *roots* (folders) configured by the user. The app tracks per-image metadata (favourite, tags, last-shown) in a sidecar SQLite database at `$XDG_DATA_HOME/superpanels/library.db` so it isn't lost on rescan.

### 7.2 Folder scanning

- Scan recursively (configurable per root).
- File-type filter: `jpg`, `jpeg`, `png`, `webp`, `avif`, `bmp`, `tiff`, `heic` (the last two via optional features). Decode failures are logged and skipped.
- The daemon watches roots with `notify` (inotify on Linux) and updates the index incrementally.
- Initial scan uses `rayon` for parallel decode of metadata (resolution, etc.). For 5,000 images on an SSD: target < 10 s.
- Thumbnails: generated lazily on first GUI request, stored under `$XDG_CACHE_HOME/superpanels/thumbs/{sha256(path)}.webp` at 320 px on the long edge. Thumbnail cache is bounded at 500 MB; oldest-not-shown evicted first.

### 7.3 Tags & favourites

- Tags are user-applied free-text strings (e.g. `nature`, `dark`, `pano`).
- Favourites are a special boolean tag with first-class UI treatment.
- Tags and favourites are stored in `library.db`, never in the image file or filename.
- Tags can be filtered on in slideshow `ImageFilters` (e.g. "rotate through favourites tagged `pano`").

### 7.4 Smart selection

For a slideshow over a folder, the picker can be configured to prefer images that suit the monitor layout:

- `aspect_ratios = "wide"` filters to images whose aspect ratio is within ±10% of the canvas aspect — i.e. images that will span well without heavy crop.
- `min_resolution` rejects anything smaller than the canvas pixel area.
- `recent_history_size = N` (default 10) suppresses the last N images shown so a 12-image folder doesn't repeat for a while.
- `tags = ["foo"]` includes only matching images.
- `favourites_only = true` is shorthand for filtering on the favourite flag.

### 7.5 Drag-and-drop

In the GUI:
- Dropping an image file onto the main window adds it to the active profile as a Single source.
- Dropping a folder adds it as a library root and activates a Folder source.
- Dropping onto a specific monitor in the canvas creates a `PerMonitor`-body profile with that file pinned to that monitor.

---

## 8. Image processing & colour

Built on the `image` crate. No `unsafe` code in our layer.

### 8.1 Operations

```rust
fn load(path: &Path) -> Result<DynamicImage>;       // returns clear error on unsupported format
fn crop(img: &DynamicImage, rect: Rect) -> DynamicImage;
fn scale(img: &DynamicImage, target: (u32, u32), filter: ScaleFilter) -> DynamicImage;
fn rotate(img: &DynamicImage, rotation: Rotation) -> DynamicImage;
fn save_temp(img: &DynamicImage, name: &str) -> Result<PathBuf>;
```

`ScaleFilter` defaults to `Lanczos3`. `Triangle` is offered for when speed matters more than quality (preview canvas — though preview never resamples the full image, see §12.3).

### 8.2 Fit modes

- `Fill` — scale until the image fills the total physical canvas, cropping the overflow. Default.
- `Fit` — letterbox/pillarbox so the entire image is visible. The user can pick the bar colour (default: black).
- `Stretch` — distort to fill exactly. Offered for completeness; rarely useful.
- `Center` — no scaling, centre the image on the canvas, crop or pad.

### 8.3 Image position offset

When `Fill` produces a canvas larger than the image area in one axis (or vice-versa), the user can slide the image along that axis via the GUI (`offset_px` IPC parameter), or via `--offset X,Y` on the CLI. Offset is per-profile and persists.

### 8.4 Colour management

v1 assumes images are in sRGB and the compositor displays sRGB. We do not embed or strip ICC profiles; we don't transform colour spaces. This is documented as a known limitation. Wide-gamut handling is a v2+ topic.

### 8.5 Temp file lifecycle

Processed per-monitor images are written to `$XDG_CACHE_HOME/superpanels/temp/`. On every apply, the temp directory is cleared *before* new files are written. The backend always receives the temp file paths, never the originals. Filenames include a content hash so a stale file isn't silently re-used.

### 8.6 Memory caps

The `image` crate decodes lazily where possible. A single decoded `DynamicImage` for an 8K wide pano is ~190 MB at 8-bit RGBA. The library never holds more than one full-res `DynamicImage` at a time per worker; processing pipelines stream where they can.

---

## 9. Profiles, schedules & slideshows

### 9.1 Profiles

A profile is the unit the user thinks in. They have profiles like "home" or "work-quiet" or "rgb-mode". Switching profiles is one click in the tray.

### 9.2 Slideshow

```rust
struct SlideshowConfig {
    interval: Duration,            // e.g. 30 minutes
    sort: SlideshowSort,           // Shuffle | Alphabetical | DateAsc | DateDesc | LastShownAsc
    recent_history_size: usize,    // suppress last N, default 10
    on_start: SlideshowStart,      // Resume | NewRandom | First
    pause_when_active: bool,       // pause the timer when the user switches images manually
    skip_on_unavailable: bool,     // if a file vanished between scan and apply, skip not error
}
```

Slideshow state (current index, history) is persisted in `$XDG_STATE_HOME/superpanels/state.json` so it survives daemon restart and reboot.

### 9.3 Schedules

Time-of-day triggers, separate from the slideshow timer:

```rust
enum Schedule {
    Daily { at: TimeOfDay, profile: String },              // e.g. "switch to dark profile at 18:00"
    Sunset { offset: Duration, profile: String },          // requires lat/long; sunset/sunrise via approximation
    Cron(String),                                          // power-user escape hatch
}
```

A profile can have a schedule, or a global schedule list can flip between profiles. Both forms are valid.

### 9.4 Manual controls

- `superpanels next` — advance the slideshow one step (works even if the daemon isn't running; falls back to in-process).
- `superpanels prev` — go back.
- `superpanels pause` / `superpanels resume`.
- All of the above are also IPC commands and are wired to the tray and GUI.

---

## 10. Backend system

### 10.1 Trait

```rust
pub trait WallpaperBackend: Send + Sync {
    fn name(&self) -> &str;
    fn availability(&self) -> Availability;
    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError>;
    fn supports_per_monitor(&self) -> bool;
}

struct AppliedReport {
    /// Number of monitors successfully assigned a wallpaper. For composite
    /// backends (GNOME) this is the count of monitors covered by the composite.
    monitors_set: usize,
    /// Wall-clock duration of the apply, including any subprocess + redraw wait.
    duration: Duration,
    /// Backend name that handled the apply (for diagnostics).
    backend: &'static str,
}

#[derive(Debug, thiserror::Error)]
enum BackendError {
    #[error("backend `{backend}` is not available: {reason}")]
    Unavailable { backend: &'static str, reason: String },
    #[error("subprocess `{cmd}` failed (exit {exit}): {stderr}")]
    Subprocess { cmd: String, exit: i32, stderr: String },
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout { cmd: String, seconds: u64 },
    #[error("D-Bus call failed: {0}")]
    DBus(String),
    #[error("monitor `{0}` not present in current layout")]
    UnknownMonitor(String),
}
```

`availability()` must be cheap: env var check or `which` lookup. Never spawn a process. The enum (defined in §6.1) lets `superpanels detect --debug` explain *why* a backend was skipped — the same diagnostic value as the detector trait.

`apply()` receives `(monitor, image)` pairs and returns an `AppliedReport` rather than `()` so future callers (the GUI toast, the daemon's audit log) can surface the count and duration without a breaking trait change. Backends that don't support per-monitor (older GNOME) composite the per-monitor crops into one large image and set it as the spanning wallpaper; `monitors_set` still reflects the number of monitors covered.

### 10.2 Auto-detection order

| Priority | Backend | Detection condition |
|---|---|---|
| 1 | KDE | `$KDE_FULL_SESSION == "true"` or `$XDG_CURRENT_DESKTOP` contains `KDE` |
| 2 | Hyprland | `$HYPRLAND_INSTANCE_SIGNATURE` set |
| 3 | Sway / wlroots | `$SWAYSOCK` set or `swww` / `swaybg` in `$PATH` |
| 4 | GNOME | `$XDG_CURRENT_DESKTOP` contains `GNOME` |
| 5 | feh | `$DISPLAY` set and `feh` in `$PATH` |
| 6 | Custom | `backend.custom_command` set in config |

User can pin a backend in config (`backend.prefer = "kde"`) to skip detection.

### 10.3 Subprocess rules (every backend follows these)

- `std::process::Command` only — never `shell = true` string concatenation.
- Always set a 10-second timeout.
- Always check `.status().success()`; return `Err` with stderr included.
- File paths are passed as `OsStr` arguments, never interpolated.
- All commands run with the inherited environment plus an explicit `LC_ALL=C` so we can parse output reliably.

### 10.4 Per-backend specifics

- **KDE.** `zbus`-backed D-Bus call to `org.kde.PlasmaShell.evaluateScript` setting per-monitor `Image` plugin source. The JS payload is a versioned template string with placeholder substitution; we generate it server-side, never accept it from user input.
- **GNOME.** `gsettings set org.gnome.desktop.background picture-uri[-dark] file://…`. Multi-monitor strategy is *to be verified in Phase 2* — modern GNOME Shell may support per-monitor wallpapers via Mutter's backend, in which case the per-monitor pipeline applies directly. The fallback (and current assumption) is to composite the per-monitor crops into a single bezel-correct image of the spanning canvas; GNOME then displays that image stretched across the desktop region. The composite is sized to the *logical* desktop, not the physical one. See PLAN Phase 2 risks.
- **Hyprland.** Uses `hyprctl hyprpaper preload` then `hyprctl hyprpaper wallpaper "MONITOR,PATH"` per monitor. We require `hyprpaper` running; we do not start it.
- **Sway/wlroots.** Prefer `swww` (smooth fades), fall back to `swaybg`. `swww img --outputs DP-1 path.png` for per-monitor.
- **feh.** `feh --bg-fill IMAGE1 IMAGE2 …` — feh handles per-monitor compositing.
- **Custom.** Shell command template from config with `{image_N}` placeholders. Runs with the same subprocess rules; user is responsible for command safety.

### 10.5 Backend feature flags

Some backends pull weight (zbus is ~1 MB compiled). We gate them behind Cargo features (`backend-kde`, `backend-gnome`, …) all on by default; minimal-distro packagers can disable some.

---

## 11. CLI interface

```
superpanels [OPTIONS] <COMMAND>

Commands:
  set         Set wallpaper immediately
  next        Advance the slideshow (or apply the next entry of the active profile's source)
  prev        Step back in the slideshow
  pause       Pause the slideshow timer
  resume      Resume the slideshow timer
  profile     Manage profiles
  library     Manage the wallpaper library
  detect      Print detected monitor layout
  daemon      Run the background daemon
  gui         Launch the graphical interface
  config      Print the resolved config (debug aid)

Global options:
  -v, --verbose    Enable debug logging (-vv for trace)
  --quiet          Suppress non-error output
  --json           Machine-readable output where supported
  --config <PATH>  Use alternate config file
  --no-daemon     Do not contact the running daemon; run in-process
```

### 11.1 `set`

```
superpanels set <IMAGE> [<IMAGE>...]
  Set wallpaper from one or more image paths.
  - One image:        spanned across all monitors with bezel compensation.
  - Multiple images:  one per monitor, left-to-right (or pin with --monitor).

Options:
  --bezel-h <MM>      Horizontal gap between monitors (mm)
  --bezel-v <MM>      Vertical gap between monitors (mm)
  --fit <MODE>        fill | fit | stretch | center  [default: fill]
  --offset <X,Y>      Image offset within the canvas (px, signed)
  --backend <NAME>    Override backend detection
  --monitors <SPEC>   Manual monitor spec (see §6.2)
  --monitor DP-1=path Pin a specific image to a specific monitor (repeatable)
  --dry-run           Process image but don't apply; print what would happen
  --save-as <NAME>    Save the resolved invocation as a profile and apply it
```

### 11.2 `profile`

```
superpanels profile list [--json]
superpanels profile show <NAME> [--json]
superpanels profile apply <NAME>
superpanels profile create <NAME> [...same options as `set`]
superpanels profile edit <NAME>      # opens $EDITOR on the profile TOML block
superpanels profile delete <NAME>
superpanels profile rename <OLD> <NEW>
superpanels profile export <NAME> [-o FILE]   # print/write a portable profile bundle
superpanels profile import <FILE>             # merge a bundle into config
```

### 11.3 `library`

```
superpanels library scan                       # rescan all configured roots
superpanels library list [--tag T] [--json]
superpanels library tag <PATH> <TAG>...
superpanels library untag <PATH> <TAG>...
superpanels library favourite <PATH>
superpanels library unfavourite <PATH>
superpanels library roots add <PATH>           # register a folder root
superpanels library roots remove <PATH>
```

### 11.4 `detect`

```
superpanels detect [--json] [--debug]

# Plain output:
# Monitor 0: DP-1     2560x1440 at (0,0)      609x343mm  108 PPI  scale 1.0
# Monitor 1: HDMI-1   1920x1080 at (2560,0)   527x296mm   83 PPI  scale 1.0  rotation: portrait
# Bezel (0→1): 8mm horizontal  (configured)

# --json: Vec<Monitor> serialised, suitable for scripting.
# --debug: also prints which detectors were tried, their stderr, and the parser output.
```

### 11.5 `daemon`

```
superpanels daemon [--foreground] [--socket PATH]
  Start the background daemon. Default is to fork to background with logs going to
  $XDG_STATE_HOME/superpanels/superpanels.log. --foreground keeps it attached
  (useful for systemd user units).
```

### 11.6 Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic failure (wallpaper not applied) |
| 2 | Bad arguments |
| 3 | Config error (invalid TOML, etc.) |
| 4 | No backend available |
| 5 | Display detection failure |
| 6 | Image processing failure (bad file, unsupported format) |
| 7 | IPC/daemon error |

---

## 12. GUI — Tauri + Svelte

### 12.1 Window layout

The main window is a three-panel layout, with a tabbed left rail (Profiles / Library / Settings).

```
┌────────────────────────────────────────────────────────────────────┐
│ Superpanels                                              [—][▢][×] │
├──────────┬─────────────────────────────────────────────────────────┤
│ Profile  │  ┌──────────────────────────────────────────────────┐   │
│ Library  │  │             Monitor preview canvas               │   │
│ Settings │  │                                                  │   │
│ ─────    │  │     ┌────────┐  ██  ┌────────┐                   │   │
│ ▶ home   │  │     │        │  ██  │        │                   │   │
│   work   │  │     │        │  ██  │        │                   │   │
│   movie  │  │     └────────┘  ██  └────────┘                   │   │
│          │  │           drag image to reposition ↕             │   │
│ [+ New]  │  └──────────────────────────────────────────────────┘   │
│          │  ┌──────────────────────────────────────────────────┐   │
│          │  │ Image:      [/home/alex/walls/pano.jpg] [Browse] │   │
│          │  │ Bezel H:    [───●─────────] 8 mm                 │   │
│          │  │ Bezel V:    [─●───────────] 5 mm                 │   │
│          │  │ Fit:        [Fill ▾]                             │   │
│          │  │ Slideshow:  [Off ▾]                              │   │
│          │  │                                  [Apply]         │   │
│          │  └──────────────────────────────────────────────────┘   │
└──────────┴─────────────────────────────────────────────────────────┘
```

### 12.2 Library view

Selectable from the left rail. Grid of thumbnails (320 px), filterable by tag, aspect ratio, and resolution. Right-click on a thumbnail: "Apply now" / "Set for monitor…" / "Add tag" / "Toggle favourite" / "Reveal in file manager". Click to focus; double-click to apply.

### 12.3 Monitor preview canvas

The canvas is the heart of the UI. Five-layer compositing:

1. **Base layer:** wallpaper image, scaled to the canvas size and positioned per the live offset.
2. **Mask layer:** dark semi-transparent overlay covering the entire canvas.
3. **Cut-out:** `destination-out` composite punches a hole through the mask for each monitor rectangle. The monitors appear as lit windows revealing the image beneath; everything outside is dimmed.
4. **Bezel bars:** solid dark rectangles between monitors, sized proportionally to the configured physical gap.
5. **Outlines & labels:** thin light borders, monitor name labels, and on-hover tooltips.

**Accuracy:**
- Monitors rendered at correct relative *physical* sizes in mm. A 27" 4K and a 24" 1080p look visibly different in the canvas.
- Correct relative positions: a monitor sitting physically lower renders lower.
- Portrait monitors rendered as rotated rectangles.
- Bezel bars sized proportionally to the configured mm gap.

**Interactivity:**
- Drag the image to reposition; on each drag tick (16 ms), an IPC `preview_crop` call returns updated `Vec<CropSpec>` and the canvas redraws.
- Bezel sliders update in real time — gap bars grow/shrink, image shifts to compensate.
- Hover a monitor: glow highlight + tooltip showing pixel range and physical-size info.
- Click a monitor: side popout showing the exact crop as it will appear on that screen.
- `R` resets the offset to centred.
- Pinch/scroll on the canvas: zoom (within 0.5×–2.0×) for inspection, doesn't change the applied result.

**Apply animation:** a fast (< 400 ms) flash per monitor confirming the apply.

**No full-res image processing on interaction.** The Rust side computes crop coordinates (pure arithmetic, microseconds). The canvas draws rectangles using the *thumbnail* of the image, scaled. The full-res pipeline only runs on Apply.

### 12.4 IPC commands (Tauri, mirrored 1:1 in the daemon's IPC)

```rust
#[tauri::command] fn detect_monitors() -> Result<Vec<Monitor>, IpcError>;
#[tauri::command] fn list_profiles() -> Result<Vec<Profile>, IpcError>;
#[tauri::command] fn apply_profile(name: String) -> Result<AppliedReport, IpcError>;
#[tauri::command] fn save_profile(profile: Profile) -> Result<(), IpcError>;
#[tauri::command] fn delete_profile(name: String) -> Result<(), IpcError>;
#[tauri::command] fn preview_crop(
    image: String,
    offset_px: (i32, i32),
    bezels: BezelConfig,
    fit: FitMode,
) -> Result<Vec<CropSpec>, IpcError>;
#[tauri::command] fn library_list(filter: LibraryFilter) -> Result<Vec<LibraryEntry>, IpcError>;
#[tauri::command] fn library_thumbnail(path: String) -> Result<Vec<u8>, IpcError>; // PNG/WebP bytes
#[tauri::command] fn library_tag(path: String, tag: String, on: bool) -> Result<(), IpcError>;
#[tauri::command] fn slideshow_next() -> Result<AppliedReport, IpcError>;
#[tauri::command] fn slideshow_prev() -> Result<AppliedReport, IpcError>;
#[tauri::command] fn slideshow_pause(paused: bool) -> Result<(), IpcError>;
#[tauri::command] fn get_config() -> Result<Config, IpcError>;
#[tauri::command] fn save_config(config: Config) -> Result<(), IpcError>;
#[tauri::command] fn redetect() -> Result<Vec<Monitor>, IpcError>;
#[tauri::command] fn current_state() -> Result<RuntimeState, IpcError>;  // active profile, current source, slideshow position
```

`IpcError` is a thin enum that flattens the typed errors from `superpanels-core` (`DetectError`, `BackendError`, `LayoutError`, `ConfigError`) into a single shape suitable for serialisation to the frontend. `Profile`, `BezelConfig`, `FitMode`, `CropSpec`, and friends are the types from §3 (after the rework — see §3.4 for `Profile`'s shape).

### 12.5 Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Enter` | Apply current settings |
| `Ctrl+N` | New profile |
| `Ctrl+S` | Save current profile |
| `Ctrl+Shift+S` | Save profile as… |
| `Ctrl+1/2/3` | Switch to top three profiles |
| `Space` | Pause/resume slideshow |
| `→` / `←` | Slideshow next / prev |
| `R` | Reset image offset |
| `F5` | Re-detect monitors |
| `Ctrl+,` | Open settings |
| `Ctrl+L` | Focus library search |
| `Esc` | Close modal / return to canvas |

### 12.6 Empty/first-run state

- First run with no monitors detected: canvas shows a placeholder with "Couldn't detect monitors — try `superpanels detect --debug`" and a button to open the manual override dialog.
- **First run with monitors detected but no physical-size config:** the canvas is rendered (so the user sees their layout) but each monitor is annotated with "physical size unknown — click to configure" and the Apply button is disabled with a tooltip explaining why. Clicking a monitor opens a small modal with two input modes:
  - *Diagonal + aspect ratio* (e.g. `27"`, `16:9`, landscape) — auto-computes physical mm. The default for "common monitor sizes" — a 27" 16:9 panel comes out to ≈ 597 × 336 mm.
  - *Direct mm entry* — width and height in mm.
  The chosen values are written to `[[monitor]]` in the config (§14.1), keyed on `stable_id` (or `name` if no stable ID).
- First run with no profile: canvas shows a single example monitor outline + "Drop an image here or pick one from the library" prompt.
- First run with no library roots: a friendly onboarding modal asks "Where are your wallpapers?" and registers a root.

### 12.7 Toasts & error surfacing

Backend errors surface as a non-blocking toast at bottom-right with the error message and a "Copy details" button. The full error (including subprocess stderr) is available in the toast's expand-disclosure and in the log file.

### 12.8 Theming

- Default dark theme, inspired by KDE Breeze Dark.
- Auto light/dark via `prefers-color-scheme`, override in settings.
- Accent colour follows `kdeglobals` `[General] AccentColor` when on KDE; otherwise a sensible blue. User can override.

---

## 13. System tray

### 13.1 Tray icon

Monochrome SVG, light and dark variants. Selected automatically based on system theme. Falls back to a 22×22 PNG on environments without SVG tray support.

### 13.2 Menu

Left click: show/hide the main window. Right click:

```
┌──────────────────────────────────┐
│ ✓ home                           │
│   work                           │
│   movie                          │
│ ─────                            │
│   ▶ Next                         │
│   ◀ Previous                     │
│   ⏸ Pause slideshow             │
│ ─────                            │
│   Open Superpanels               │
│   Settings…                      │
│ ─────                            │
│   Quit                           │
└──────────────────────────────────┘
```

The tick mark next to the active profile updates live when the daemon switches profiles.

### 13.3 Tooltip

Hovering the tray icon shows: `Superpanels — <profile name> — <current filename>`.

### 13.4 Notifications

Optional desktop notifications (off by default, opt-in in settings) on:
- Apply success (briefly).
- Apply failure (always, even when notifications-on-success is off).
- Slideshow advanced (off by default).

Uses `notify-rust` / `org.freedesktop.Notifications`.

---

## 14. Configuration & state

### 14.1 Config file

Location: `$XDG_CONFIG_HOME/superpanels/config.toml` (default: `~/.config/superpanels/config.toml`).

Parsed with `serde` + `toml`. All fields have sane defaults so a minimal config (or no config) is valid.

```toml
[general]
default_profile  = "home"
autostart        = true              # write desktop file on first run
notifications    = "errors"          # off | errors | all
theme            = "auto"            # auto | light | dark

[backend]
prefer           = "auto"            # auto | kde | gnome | sway | hyprland | feh | custom
custom_command   = ""                # only when prefer = "custom"; supports {image_N}, {monitor_N}

[library]
roots            = ["~/Pictures/walls"]
recursive        = true
thumbnail_size   = 320
auto_scan        = true              # rescan on FS change

# Per-monitor physical sizes. The detector gives us pixels; this gives us
# millimetres. Match by stable_id when the detector supplied one (KDE per-output
# UUID etc.); fall back to name for compositors that don't expose a stable ID.
# At least one of `stable_id` / `name` must be set. The GUI's first-run flow
# writes these blocks for you.
[[monitor]]
stable_id     = "f7f0f124-9e9b-4ef0-91a7-426d58091760"  # KDE UUID
name          = "DP-1"                                  # informational; match falls back to this
physical_mm   = [597, 336]                              # 27" 16:9 landscape

[[monitor]]
name          = "HDMI-A-1"
physical_mm   = [527, 296]                              # 24" 16:9

[[profile]]
name = "home"
bezels = { horizontal_mm = 8.0, vertical_mm = 5.0 }

[profile.body]
type   = "span"
fit    = "fill"
offset = [0, 0]

[profile.body.source]
type = "single"
path = "~/walls/pano.jpg"

[[profile]]
name = "work"
bezels = { horizontal_mm = 8.0, vertical_mm = 5.0 }

[profile.body]
type = "span"
fit  = "fill"

[profile.body.source]
type = "slideshow"

[profile.body.source.images]
type      = "folder"
path      = "~/walls/work"
recursive = false
filters   = { aspect_ratios = "wide" }

[profile.body.source.config]
interval_secs       = 1800
sort                = "shuffle"
recent_history_size = 10
on_start            = "resume"
```

Enums use `serde`'s tagged representation (`#[serde(tag = "type", rename_all = "snake_case")]`) so the TOML stays readable and round-trip-stable. This is the source of truth for the on-disk format; the Rust types in §3 are the source of truth for the runtime model.

### 14.2 Validation

Config is validated at load time. Invalid configs *do not crash*; they return an error with the exact field path (`profile[1].slideshow.interval_secs: must be > 0`) and the previous wallpaper remains.

### 14.3 Hot reload

On `SIGHUP` the daemon reloads config. The CLI does not need this — it loads config fresh on each invocation. The GUI's "Save" button writes the file and triggers reload via IPC.

### 14.4 Runtime state

Location: `$XDG_STATE_HOME/superpanels/state.json` (default: `~/.local/state/superpanels/state.json`).

```json
{
  "active_profile": "home",
  "current_source": { "kind": "single", "path": "~/walls/pano.jpg" },
  "slideshow": {
    "profile": "home",
    "current_index": 17,
    "history": ["walls/a.jpg", "walls/b.jpg"],
    "paused": false
  },
  "last_apply": "2026-04-26T15:42:00Z",
  "version": 1
}
```

`current_source` records the *source* (a serialised `SpanSource` or `PerMonitorProfile.assignments`), never the per-monitor temp file paths — those are wiped at the start of each apply (§8.5) so persisting them would always be stale. If the daemon needs to repaint after a re-detection, it re-runs the pipeline from the source. State is restored on daemon start so the slideshow doesn't loop back to the start after every reboot.

### 14.5 Library DB

SQLite, location `$XDG_DATA_HOME/superpanels/library.db`. Schema versioned via `PRAGMA user_version`; migrations are pure-Rust, idempotent, applied on startup. Tables: `entries`, `tags`, `entry_tags`, `roots`.

### 14.6 Migration

Each persistent file (`config.toml`, `state.json`, `library.db`) carries a `version` field. On load, if the version is older than the binary expects, a migration step runs and a backup is left at `<file>.v<N>.bak`. If the version is newer (downgrade), the binary refuses to write and prints a clear error.

---

## 15. Logging & observability

### 15.1 Logging

- `tracing` + `tracing-subscriber`.
- Default level: `info` for app code, `warn` for dependencies.
- Console output is human-friendly with colour (when stdout is a TTY).
- File output is JSON-lines, written to `$XDG_STATE_HOME/superpanels/superpanels.log`, rotated daily, kept for 7 days.
- `-v` / `--verbose` raises level to `debug`; `-vv` to `trace`.
- A redaction layer scrubs anything that looks like a home path from JSON output (`/home/alex/...` → `~/...`) for shareable logs.

### 15.2 Crash diagnostics

`color-eyre` for panics in user-facing binaries, `human-panic` for end-user-friendly crash messages. A panic dumps a structured report to `$XDG_STATE_HOME/superpanels/crash-<ts>.txt` and prints a path to it.

### 15.3 Telemetry

None. Superpanels does not phone home. There is no analytics, no usage reporting, no crash uploads. (We may add an opt-in `--report-crash` flag in a later release that the user runs explicitly to attach a crash report to a GitHub issue.)

---

## 16. Error handling philosophy

- **No panics in library code.** `unwrap()` / `expect()` banned outside tests and `main()`.
- All fallible functions return `anyhow::Result<T>` (binary code) or `thiserror`-typed errors (library code, where the error variant is part of the API).
- Error messages are written for the *end user*, not the developer. They say what happened, why, and what to try next.
- Subprocess failures include the command run and its stderr.
- Config parse errors include the file path and the field path.
- Display detection failure is non-fatal — the user can provide manual specs.
- Backend failures revert no state — we set the wallpaper or we don't; we never half-apply.

---

## 17. Security & sandbox considerations

- Image decoding happens in a worker thread. The `image` crate is safe Rust but image files come from untrusted sources (downloads), so we cap decode memory at a configurable limit (default 512 MB; rejects pathological PNG bombs).
- Custom backend commands are user-supplied; we run them as the user. The config doc warns clearly; the GUI's custom-command field shows a "this runs with your privileges" callout.
- No HTTP fetching in v1.
- **Tauri v2 hardening.** The defaults are not safe enough for an app that touches user-supplied paths and runs subprocesses. We lock down four surfaces:
  - **CSP.** `tauri.conf.json` sets `app.security.csp` explicitly:
    ```
    default-src 'self';
    script-src  'self';
    style-src   'self' 'unsafe-inline';   /* inline style="" attrs for canvas positioning */
    img-src     'self' data: blob:;       /* thumbnails arrive as bytes via IPC, become blob: URLs */
    connect-src 'self' ipc: http://ipc.localhost;  /* Tauri v2 IPC transport */
    object-src  'none';
    base-uri    'self';
    frame-ancestors 'none';
    ```
    No `'unsafe-eval'`, no `'unsafe-inline'` on `script-src`. `'unsafe-inline'` on `style-src` is the one concession — Svelte's runes-based canvas positioning sets `style="--offset: {x}px"` on elements, which CSP3 governs via `style-src-attr` (falling back to `style-src`). If we eliminate inline-attr styles in Phase 4a we drop this too.
  - **`withGlobalTauri: false`.** No `window.__TAURI__` global; everything goes through `import { invoke } from '@tauri-apps/api/core'`. Smaller attack surface from page scripts.
  - **Capabilities (least-privilege per window).** Plugin permissions are declared in `src-tauri/capabilities/default.json` against the `main` window only. We grant exactly:
    - `core:default` (window resize, drag, etc.).
    - `fs:scope` *constrained at runtime* to the configured library roots plus `$XDG_CACHE_HOME/superpanels/thumbs/`. No blanket `fs:allow-read-file`.
    - `dialog:allow-open` for the file picker.
    - `notification:allow-notify` only if the user opted in.
    - **No** `shell:`, `http:`, `process:`, or `os:execute` permissions. Subprocesses run in our Rust code with our own subprocess rules (§10.3), never via the shell plugin.
  - **No `asset:` protocol for arbitrary paths.** Thumbnails and previews go through a dedicated IPC command (`library_thumbnail`) that returns bytes; the frontend wraps them as `blob:` URLs. This means a webview script can't read arbitrary files via `asset://` even if CSP is bypassed.
- Custom IPC commands (the ones in §12.4) are app-level handlers — Tauri's capability system gates *plugin* permissions, not custom `#[tauri::command]`s. We mitigate that by validating every command's input as if it were untrusted: paths are canonicalised and verified to be inside an allowed root before any FS access; profile names are matched against the on-disk profile list, never used as path components.
- No `unsafe` Rust in our crates. Allowed via `#![forbid(unsafe_code)]`.

---

## 18. Accessibility & i18n

### 18.1 Accessibility

- All UI controls have ARIA labels.
- Full keyboard navigation — every action reachable from the canvas is also reachable from a focused control.
- Focus indicators are visible and high-contrast.
- Colour isn't the only signal: the canvas's "active monitor" highlight is shape + label, not just glow.
- Honours `prefers-reduced-motion`: apply animation is replaced by an instant state change.

### 18.2 Internationalisation

- All UI strings and CLI human-readable messages flow through a `t!` macro backed by `fluent`.
- v1 ships with English. The string catalogue is structured so adding a locale is purely a translation task; no UI code changes.
- Numbers, dates, units (mm) use `icu` for locale-aware formatting.

---

## 19. Performance targets

| Operation | Target | Measured on |
|---|---|---|
| `superpanels set <single image>` end-to-end (excl. compositor redraw) | < 500 ms | 4K image, 3-monitor setup, NVMe |
| `superpanels detect` | < 200 ms | KDE Wayland on Ryzen 5600 |
| Canvas drag → redraw frame | < 8 ms (≥ 120 fps) | Ryzen 5600 / iGPU |
| Library scan, 5,000 images | < 10 s cold | NVMe |
| Library scan, 5,000 images | < 1 s warm (cached metadata) | NVMe |
| Thumbnail generation, single 4K image | < 200 ms | Ryzen 5600 |
| Daemon idle CPU | < 0.1% | Any |
| Daemon resident memory, idle, 1k library | < 60 MB | Any |

Performance regressions are tracked in `criterion` benchmarks under `crates/superpanels-core/benches/`.

---

## 20. Testing strategy

### 20.1 Layers

- **Unit tests** in each module. Bezel math is the most heavily tested; every documented edge case in §4.5 has at least one test.
- **Snapshot tests** for parsers (kscreen-doctor, xrandr, hyprctl) using captured real-world fixtures under `crates/superpanels-core/tests/fixtures/`. `insta` for diffs.
- **Property tests** with `proptest` for bezel math: random monitor layouts, random image sizes — invariants like "sum of crop widths == canvas pixel width" and "no two crops overlap" must hold.
- **Integration tests** with a `MockBackend` that records `apply()` calls instead of touching the desktop. The whole `set` pipeline runs against this in CI.
- **Golden-image tests** for the image processing pipeline: "given input image X and config Y, the file written for monitor 0 has SHA256 Z." Catches regressions in scaling/rotation/cropping.
- **Manual smoke tests** before each release on KDE, GNOME, Sway, Hyprland — see `docs/release-checklist.md`.

### 20.2 What is NOT auto-tested

- The GUI canvas rendering pipeline visually. We assert the IPC outputs, not the rendered pixels — that's the user's job to verify.
- The compositor actually painting the wallpaper. `MockBackend` proves we sent the right paths; we trust the compositor.

### 20.3 CI

GitHub Actions matrix:
- `ubuntu-22.04` and `ubuntu-24.04` (proxy for Linux variance — Arch builds are tested via the AUR PKGBUILD on tag).
- `cargo test --workspace --all-features`.
- `cargo clippy --workspace --all-features -- -D warnings`.
- `cargo fmt --check`.
- `cargo audit` weekly.
- `cargo deny check` for licence policy.
- `cargo bench` smoke run (build only) on PR; full bench with regression check on main.

---

## 21. Packaging & distribution

### 21.1 Arch / CachyOS (primary target)

**CLI-only PKGBUILD** (`superpanels`):
```
makedepends=(rust)
depends=()                           # zero runtime deps; statically linked where viable
```

**GUI PKGBUILD** (`superpanels-gui`):
```
makedepends=(rust nodejs npm)
depends=(webkit2gtk-4.1)             # Tauri's only Linux runtime dep
```

WebKitGTK is already present on KDE/GNOME systems; on minimal installs it's the only addition. Both packages are submitted to the AUR. We aim for `extra/` inclusion once stable.

### 21.2 Crates.io

The CLI is published as `superpanels`. `cargo install superpanels` works without additional setup.

### 21.3 Flatpak

A Flatpak manifest under `packaging/flatpak/` for non-Arch distros. Not the primary distribution channel; provided for breadth.

### 21.4 GitHub Releases

Pre-built binaries attached to each release tag:
- `superpanels-x86_64-linux-cli` — statically linked where viable; glibc ≥ 2.17.
- `superpanels-x86_64-linux-gui.tar.zst` — includes Tauri app bundle.
- `superpanels-aarch64-linux-cli` — for ARM SBC users.

CI via GitHub Actions: `cargo build --release` + Tauri bundler on each tag push, attached automatically.

### 21.5 Versioning

SemVer. `0.x.y` until the config schema is frozen; `1.0.0` when the schema is stable enough that we'll write migration code rather than break it. Pre-1.0 minor bumps may include breaking changes; the changelog is explicit.

---

## 22. Out of scope (v1)

- **Windows / macOS** support — the backend trait and detector trait are shaped to allow it; not a v1 deliverable.
- **Perspective correction** for toed-in monitors — a Superpaper feature; uncommon, complex; v2 if requested.
- **Per-monitor colour calibration / ICC profile management** — v2.
- **Live wallpaper / video / shader wallpapers** — out of scope indefinitely; a different app's job.
- **Online wallpaper sources** (wallhaven, Unsplash, Reddit) — possibly v2 as an opt-in plugin.
- **Wallpaper editing** beyond canvas-fitting — Krita/GIMP territory.
- **Multi-user / multi-seat** — single user, single Wayland session.
- **Mobile / KDE Plasma Mobile** — out of scope.
- **AI-driven scene composition** — out of scope.

---

## 23. Open questions

These need resolution before or during early implementation. Tracked as GitHub issues once the repo is public.

1. **Hyprland integration.** Should we support `swww` on Hyprland in addition to `hyprpaper`? Many Hyprland users prefer it. Probably yes — list both, pick the one running.
2. **GNOME multi-monitor span.** GNOME's `picture-uri` is one image per workspace, stretched. Our composite-to-one approach works, but GNOME users with very large total resolutions (24-megapixel composite for a 6K + 4K + 4K trio) will see a memory spike. Acceptable? Cap at 8K composite and downscale?
3. **Edid hashing.** Should we hash the full EDID or just `manufacturer + model + serial`? Latter is more stable (cable swap doesn't change the hash); former is more unique.
4. **Tauri v2 vs Iced.** Tauri brings WebKitGTK as a dep. Iced is pure Rust, smaller binaries, but the canvas work is more code. Decision: stick with Tauri for v1 (web tech for the canvas is hard to beat); revisit if WebKitGTK is a deal-breaker for a packager.
5. **Slideshow during sleep.** Should the slideshow timer pause when the screen is locked / the system is asleep? Answer: yes, listen on `org.freedesktop.login1` for `PrepareForSleep` and `LockedHint`.
6. **Schema for per-monitor profiles.** When a profile pins images per monitor, how do we refer to monitors in a way that survives re-plugs? `MonitorRef { stable_id?, name? }` (§6.4) is the resolved design — `stable_id` is the KDE per-output UUID where available, an EDID-derived hash otherwise. Needs a real-world test on non-KDE compositors.
