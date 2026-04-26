# Superpanels — Technical Specification

> Multi-monitor wallpaper spanning with bezel correction.
> Single binary. Zero runtime dependencies. Looks great.

---

## 1. Goals

- Take a wide or panoramic image and span it across multiple monitors so the image content is continuous across physical screens — bezels included.
- Bezel-aware: the image accounts for the physical gap between screens so content isn't shifted or duplicated.
- Trivially installable: `pacman -S superpanels` or `cargo install superpanels`. No Python, no virtualenvs, no system library conflicts.
- Beautiful optional GUI via Tauri + Svelte with a live monitor preview canvas.
- CLI-first: the full feature set is available headless for scripting and automation.
- Extensible backends: each desktop environment is an isolated, testable module.

---

## 2. Core Concepts

### Monitor
A physical display as reported by the system.

```
Monitor {
    id: u32                   // ordering index (left to right, top to bottom)
    name: String              // e.g. "DP-1", "HDMI-A-1"
    position: (i32, i32)      // top-left corner in the logical desktop (px)
    resolution: (u32, u32)    // pixel dimensions (w, h)
    physical_size: (u32, u32) // physical dimensions in mm (w, h)
    scale: f64                // HiDPI scale factor (1.0, 1.5, 2.0, ...)
    ppi: f64                  // derived: pixels per inch
}
```

### BezelConfig
Physical gap sizes between adjacent screens, specified in millimetres. Stored per-profile, defaulting to zero (clean span with no gap compensation).

```
BezelConfig {
    between_monitors: HashMap<(MonitorId, MonitorId), GapMm>
}
```

Or simplified: a single `horizontal_mm` and `vertical_mm` applied uniformly (covers 90% of real setups).

### CropSpec
The rectangle within the source image that maps to a given monitor after bezel compensation.

```
CropSpec {
    monitor_id: u32
    src_rect: Rect  // (x, y, w, h) in source image pixels
}
```

### Profile
A named, persistent configuration.

```
Profile {
    name: String
    images: ImageSource          // single path, directory, or per-monitor list
    mode: SpanMode               // Span | Individual | Slideshow
    bezels: BezelConfig
    slideshow: Option<SlideshowConfig>
    backend_override: Option<BackendKind>
}
```

---

## 3. The Bezel Math

This is the core of the project. The principle: **the image maps to physical screen space, including the space occupied by bezels.**

### Simple case — two identical monitors, uniform gap

```
Physical layout (mm):   [==monitor 1==][bezel|bezel][==monitor 2==]
                         <--- W1 mm ---> <-- G mm --> <--- W2 mm --->

Total physical width = W1 + G + W2 (mm)
Image pixels per mm  = image_width_px / total_physical_width_mm

Monitor 1 crop:
  x = 0
  w = W1_mm * px_per_mm

Monitor 2 crop:
  x = (W1 + G) * px_per_mm
  w = W2_mm * px_per_mm
```

The gap `G` represents real physical space — image content exists there but is hidden behind the bezels. The crops for monitor 1 and monitor 2 are non-overlapping; the gap content is simply not rendered.

### Mixed PPI case

When monitors have different pixel densities, normalise to a reference PPI first (the maximum PPI across all monitors, or a user-configured value). Each monitor's logical pixel count is scaled to the reference PPI before the crop is calculated. This ensures the image appears at the same physical size on all screens.

### General algorithm

```
1. Collect monitors, sort left-to-right (then top-to-bottom for rows).
2. Convert each monitor's physical width to px at reference PPI.
3. Convert each bezel gap from mm to px at reference PPI.
4. Build a 1D (or 2D for grid layouts) physical pixel map:
   monitor_physical_starts[i] = sum of (monitor_widths + gap_widths) before i
5. total_physical_width = sum of all monitor widths + all gap widths
6. Scale source image so image_width == total_physical_width (or crop/fit per mode).
7. For each monitor i:
     src_x = monitor_physical_starts[i]
     src_w = monitor_physical_widths[i]   // in reference PPI pixels
     crop  = scale src_rect back to actual image pixels if image was scaled
```

For a 2D grid layout (e.g. 2×2 monitors), the same logic applies independently per row and per column, producing a 2D crop rectangle per monitor.

---

## 4. Architecture

```
superpanels/
├── Cargo.toml
├── Cargo.lock
├── src/                          ← Rust core (pure logic, no UI)
│   ├── main.rs                   ← CLI entry point (clap)
│   ├── lib.rs                    ← public API surface (also used by Tauri)
│   ├── display/
│   │   ├── mod.rs                ← Monitor struct, detection orchestration
│   │   ├── wayland.rs            ← wlr-randr / kscreen-doctor parsing
│   │   └── x11.rs                ← xrandr parsing
│   ├── layout.rs                 ← BezelConfig, CropSpec, the bezel math
│   ├── image.rs                  ← Pillow equivalent: slice, scale, composite
│   ├── config.rs                 ← TOML profile load/save (serde)
│   └── backends/
│       ├── mod.rs                ← WallpaperBackend trait + auto-detection
│       ├── kde.rs                ← zbus dbus → KDE PlasmaShell
│       ├── gnome.rs              ← gsettings subprocess
│       ├── sway.rs               ← swaybg / swww
│       ├── hyprland.rs           ← hyprctl hyprpaper
│       ├── feh.rs                ← X11 fallback
│       └── custom.rs             ← user-supplied shell command
├── src-tauri/                    ← Tauri shell
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       └── main.rs               ← #[tauri::command] wrappers around lib.rs
└── ui/                           ← Svelte 5 frontend
    ├── package.json
    ├── vite.config.ts
    └── src/
        ├── App.svelte
        ├── lib/
        │   ├── MonitorCanvas.svelte     ← live preview
        │   ├── ProfileList.svelte
        │   ├── BezelControls.svelte
        │   └── ImagePicker.svelte
        └── stores/
            └── profile.ts
```

### Separation of concerns

- `src/lib.rs` is pure logic. No Tauri, no UI, no globals. Fully testable.
- `src/main.rs` is a thin CLI wrapper around `lib.rs`.
- `src-tauri/src/main.rs` is a thin Tauri wrapper around `lib.rs`.
- The UI communicates with the backend exclusively via Tauri's typed IPC commands.

---

## 5. Backend System

### Trait

```rust
pub trait WallpaperBackend: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn apply(&self, images: &[PathBuf]) -> anyhow::Result<()>;
}
```

`is_available()` must be cheap: check an env var or call `which`. Never spawn a process.

`apply()` receives one `PathBuf` per monitor, in left-to-right order. The backend is responsible for knowing what to do with them (KDE sets per-monitor, feh composites them).

### Auto-detection order

| Priority | Backend | Detection condition |
|---|---|---|
| 1 | KDE | `$KDE_FULL_SESSION == true` or `$XDG_CURRENT_DESKTOP == KDE` |
| 2 | GNOME | `$XDG_CURRENT_DESKTOP` contains `GNOME` |
| 3 | Hyprland | `$HYPRLAND_INSTANCE_SIGNATURE` set |
| 4 | Sway/wlroots | `$SWAYSOCK` set or `swww` / `swaybg` in PATH |
| 5 | feh | `$DISPLAY` set and `feh` in PATH |
| 6 | Custom | `backend.custom_command` set in config |

User can pin a backend in config to skip detection entirely.

### Subprocess rules (all backends must follow)

- Use `std::process::Command` — never `shell = true` string concatenation.
- Always set `.timeout(Duration::from_secs(10))`.
- Always check `.status().success()` and return `Err` if not.
- Always capture `.stderr()` and include it in the error message.
- File paths are passed as `OsStr` arguments, never interpolated into strings.

---

## 6. Display Detection

Detection is attempted in order, stopping at the first success:

1. **KDE**: `kscreen-doctor -o` — gives monitor names, resolutions, positions, and physical sizes in mm. Preferred on KDE because it reports accurate physical dimensions.
2. **wlr-randr**: works on any wlroots compositor (Sway, Hyprland, etc.).
3. **xrandr**: X11 fallback. Parse `--listmonitors` and `--verbose` for physical sizes.
4. **$MONITOR env** fallback: Hyprland exposes monitor data via `hyprctl monitors -j` (JSON).

Each detector runs as a subprocess with a **5 second timeout**. If all fail, return a clear error: `"Could not detect monitor layout. Try --monitors to specify manually."`.

Manual override via CLI: `--monitors 1920x1080+0+0,1920x1080+1920+0` for scripting environments.

---

## 7. Image Processing

Built on the `image` crate. No unsafe code.

### Operations

```
load(path: &Path) -> DynamicImage
crop(img: &DynamicImage, rect: Rect) -> DynamicImage
scale_to_fit(img: &DynamicImage, target: (u32, u32), mode: FitMode) -> DynamicImage
save_temp(img: &DynamicImage) -> PathBuf   // writes to $XDG_CACHE_HOME/superpanels/
```

### Fit modes

- `Fill` — scale until the image fills the total physical canvas, cropping excess. No empty space.
- `Fit` — scale until the image fits within the canvas, with letterbox/pillarbox if needed.
- `Stretch` — distort to fill exactly. Usually looks bad, but offered for completeness.
- `Center` — no scaling, centre the image, crop or pad as needed.

### Temp file lifecycle

Processed images are written to `$XDG_CACHE_HOME/superpanels/temp/`. On next run, the temp directory is cleared before new files are written. The backend always receives the temp file paths, never the originals.

---

## 8. Configuration

Location: `$XDG_CONFIG_HOME/superpanels/config.toml` (default: `~/.config/superpanels/config.toml`).

Parsed with `serde` + `toml`. All fields have sane defaults so a minimal config is valid.

```toml
[general]
default_profile = "home"

[backend]
prefer = "auto"        # auto | kde | gnome | sway | hyprland | feh | custom
custom_command = ""    # only used when prefer = "custom"
                       # use {image_N} placeholders, e.g. "feh --bg-fill {image_0} {image_1}"

[[profile]]
name = "home"
images = ["/home/user/walls/panorama.jpg"]
mode = "span"          # span | individual | slideshow

[profile.bezels]
horizontal_mm = 8.0    # gap between horizontally adjacent monitors
vertical_mm = 5.0      # gap between vertically adjacent monitors

[[profile]]
name = "work"
images = ["/home/user/walls/"]   # directory: picks randomly unless slideshow
mode = "slideshow"

[profile.slideshow]
interval_secs = 600
sort = "shuffle"       # shuffle | alphabetical | date_asc | date_desc
```

Config is validated at load time with friendly error messages pointing to the exact field. Invalid configs never crash — they return an error and the previous wallpaper remains.

---

## 9. CLI Interface

```
superpanels [OPTIONS] <COMMAND>

Commands:
  set      Set wallpaper immediately
  profile  Manage profiles
  detect   Print detected monitor layout
  daemon   Run in background (slideshow, tray if GUI feature enabled)
  gui      Launch the graphical interface

Options:
  -v, --verbose    Enable debug logging
  --config <PATH>  Use alternate config file

superpanels set <IMAGE> [<IMAGE>...]
  Set wallpaper from one or more image paths.
  One image: spanned across all monitors with bezel compensation.
  Multiple images: one per monitor, left to right.

  --bezel-h <MM>      Horizontal gap between monitors (mm)
  --bezel-v <MM>      Vertical gap between monitors (mm)
  --fit <MODE>        fill | fit | stretch | center  [default: fill]
  --backend <NAME>    Override backend detection
  --monitors <SPEC>   Manual monitor spec: WxH+X+Y,WxH+X+Y,...
  --dry-run           Process image but don't apply; print what would happen

superpanels profile list
superpanels profile apply <NAME>
superpanels profile edit <NAME>      # opens $EDITOR on the profile TOML block
superpanels profile delete <NAME>

superpanels detect
  # Output example:
  # Monitor 0: DP-1    2560x1440 at (0, 0)      609x343mm  108 PPI  scale 1.0
  # Monitor 1: HDMI-1  1920x1080 at (2560, 0)   527x296mm   83 PPI  scale 1.0
  # Gap (0→1): 16mm horizontal
```

---

## 10. GUI — Tauri + Svelte

### System tray

- Icon: monochrome SVG, adapts to light/dark system theme.
- Left click: show/hide main window.
- Right click menu:
  - Profile list (tick marks active profile)
  - Separator
  - Next wallpaper (if slideshow active)
  - Settings
  - Quit

### Main window

Three-panel layout:

```
┌──────────────────────────────────────────────────────────┐
│  [Profile list]  │  [Monitor preview canvas]             │
│                  │                                        │
│  > home          │  ┌────────┐  ██  ┌────────┐           │
│    work          │  │        │  ██  │        │           │
│    gaming        │  │        │  ██  │        │           │
│                  │  └────────┘  ██  └────────┘           │
│  [+ New]         │     drag image to reposition ↕         │
│                  ├────────────────────────────────────────│
│                  │  [Image path / picker]                 │
│                  │  Bezel H: [──●──────] 8mm              │
│                  │  Bezel V: [──●──────] 5mm              │
│                  │  Fit:     [Fill ▼]                     │
│                  │                           [Apply]      │
└──────────────────┴────────────────────────────────────────┘
```

### Monitor preview canvas

The canvas uses layered compositing to give an immediate, accurate visual of exactly what the wallpaper will look like across all screens:

1. **Base layer:** wallpaper image, scaled and positioned across the full canvas area.
2. **Mask layer:** dark semi-transparent overlay covering the entire canvas.
3. **Cut-out:** `destination-out` composite punches a hole through the mask for each monitor rectangle. The monitors appear as lit windows showing the image beneath — everything outside the monitors is dimmed.
4. **Bezel bars:** solid dark rectangles between monitors, sized proportionally to the configured physical gap.
5. **Outlines:** thin light border around each monitor rectangle.

**Accuracy:**
- Monitors rendered at correct relative physical sizes in mm — a 27" 4K and a 24" 1080p look visibly different in the canvas.
- Correct relative positions: if one monitor sits physically lower, it renders lower.
- Portrait monitors rendered at correct rotated orientation.

**Interactivity:**
- Drag image to reposition. On each drag tick, `preview_crop` IPC call returns updated `Vec<CropSpec>` from Rust; canvas redraws immediately.
- Bezel sliders update the canvas in real time — gap bars grow/shrink and the image shifts to compensate.
- Hover a monitor: glow highlight + tooltip showing the pixel range from the source image.
- Click a monitor: popout showing the exact crop region as it will appear on that screen.
- On Apply: brief flash animation per monitor confirming the wallpaper was set.

Renders at canvas resolution only — no full-res image processing during interaction. The Rust side computes crop coordinates (pure arithmetic, microseconds); the canvas just draws rectangles.

### IPC commands (Tauri)

```rust
#[tauri::command] detect_monitors() -> Result<Vec<Monitor>>
#[tauri::command] list_profiles() -> Result<Vec<Profile>>
#[tauri::command] apply_profile(name: String) -> Result<()>
#[tauri::command] save_profile(profile: Profile) -> Result<()>
#[tauri::command] delete_profile(name: String) -> Result<()>
#[tauri::command] preview_crop(image: String, profile: Profile) -> Result<Vec<CropSpec>>
#[tauri::command] get_config() -> Result<Config>
#[tauri::command] save_config(config: Config) -> Result<()>
```

---

## 11. Error Handling Philosophy

- **No panics in library code.** `unwrap()` and `expect()` are banned outside tests and `main()`.
- All functions that can fail return `anyhow::Result<T>`.
- Error messages are written for the end user, not the developer. They say what happened, why, and what to try next.
- Subprocess failures include the command that was run and its stderr output.
- Config parse errors include the file path and line number.
- Display detection failure is non-fatal — the user can provide manual specs.

---

## 12. Packaging

### Arch / CachyOS (primary target)

**CLI-only PKGBUILD:**
```
makedepends=(rust)
depends=()           # zero runtime deps — fully static where possible,
                     # or dynamically linked to glibc only
```

**GUI PKGBUILD (Tauri):**
```
makedepends=(rust nodejs npm)
depends=(webkit2gtk-4.1)   # Tauri's only runtime dep on Linux
```

WebKitGTK is already present on any KDE/GNOME system. On a minimal install it's the only addition.

### Crates.io

The CLI is published as a crate. `cargo install superpanels` works with no additional setup.

### GitHub Releases

Pre-built binaries attached to each release tag:
- `superpanels-x86_64-linux-cli` (statically linked, glibc >= 2.17)
- `superpanels-x86_64-linux-gui.tar.zst` (includes Tauri app bundle)

CI via GitHub Actions: `cargo build --release` + Tauri bundler on each tag push.

---

## 13. What's Explicitly Out of Scope

- Windows and macOS support (can be added later — the backend trait makes it mechanical)
- Perspective correction (Superpaper feature, uncommon, complex — add post-MVP if wanted)
- Per-monitor colour calibration
- Live wallpaper / video
- Wallpaper download from online sources
