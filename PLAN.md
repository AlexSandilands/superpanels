# Superpanels — Development Plan

---

## Guiding principles

- Build vertically, not horizontally. Each phase ships something that actually works end-to-end.
- No placeholder code. If it's written, it's correct and tested.
- The GUI is added on top of a working CLI — never the other way round.
- Keep the diff reviewable. Small, focused commits.

---

## Phase 1 — Core Rust library (CLI MVP)

**Goal:** `superpanels set panorama.jpg` works on KDE Wayland and produces a correctly sliced wallpaper.

**Milestone:** Can be handed to someone else on KDE and it works first try.

### 1.1 Project scaffold
- [ ] `cargo new superpanels --lib`
- [ ] Add to `Cargo.toml`: `image`, `anyhow`, `serde`, `toml`, `clap`
- [ ] Set up workspace structure (`src/lib.rs`, `src/main.rs`, submodules)
- [ ] Configure `rust-analyzer`, `clippy`, `rustfmt`
- [ ] `.gitignore`, `LICENSE`, basic `README.md`

### 1.2 Display detection (`src/display/`)
- [ ] `Monitor` struct with all fields
- [ ] KDE: parse `kscreen-doctor -o` output → `Vec<Monitor>`
- [ ] X11 fallback: parse `xrandr --listmonitors`
- [ ] Manual override: parse `--monitors WxH+X+Y,WxH+X+Y` string
- [ ] Subprocess wrapper: timeout 5s, capture stderr, return `anyhow::Result`
- [ ] `superpanels detect` CLI command printing monitor table

### 1.3 Bezel layout math (`src/layout.rs`)
- [ ] `BezelConfig` struct
- [ ] `compute_crop_specs(monitors: &[Monitor], bezels: &BezelConfig) -> Vec<CropSpec>`
- [ ] Handle uniform single-row layout (most common case first)
- [ ] Handle mixed PPI: normalise to max PPI before calculation
- [ ] Handle 2D grid layout (e.g. 2×2 monitors)
- [ ] Unit tests covering: identical monitors, mixed PPI, single monitor, 3+ monitors in a row

### 1.4 Image processing (`src/image.rs`)
- [ ] `load(path) -> DynamicImage` with clear error on unsupported format
- [ ] `scale_to_fit(img, total_canvas_size, FitMode) -> DynamicImage`
- [ ] `crop(img, rect) -> DynamicImage`
- [ ] `save_temp(img, index) -> PathBuf` (to `$XDG_CACHE_HOME/superpanels/temp/`)
- [ ] Clear temp directory at start of each run
- [ ] Integration test: load a real image, crop it, verify output dimensions

### 1.5 KDE backend (`src/backends/kde.rs`)
- [ ] `WallpaperBackend` trait defined in `src/backends/mod.rs`
- [ ] `KdeBackend::is_available()` checks `$KDE_FULL_SESSION`
- [ ] `KdeBackend::apply(images)` calls KDE PlasmaShell via `zbus`
- [ ] Sets per-monitor wallpaper (one image path per screen)
- [ ] Subprocess/dbus rules: timeout, error capture, return `Err` on failure
- [ ] Manual test on KDE: does it actually set the wallpaper?

### 1.6 Auto-detection + config (`src/backends/mod.rs`, `src/config.rs`)
- [ ] Backend auto-detection in priority order
- [ ] `Config` and `Profile` structs with `serde` derives
- [ ] Load from `~/.config/superpanels/config.toml`, create defaults if missing
- [ ] Validate on load, return user-friendly errors for bad fields

### 1.7 CLI wiring (`src/main.rs`)
- [ ] `superpanels set <IMAGE> [--bezel-h N] [--bezel-v N] [--backend NAME]`
- [ ] `superpanels detect`
- [ ] `--dry-run` flag: print crop specs, skip file write and backend call
- [ ] `--verbose` flag: debug logging via `tracing`

---

## Phase 2 — Additional backends

**Goal:** Works on GNOME and basic X11 setups too. Other Wayland compositors covered.

**Milestone:** Someone on GNOME or Sway can use it without config changes.

- [ ] `GnomeBackend`: `gsettings set org.gnome.desktop.background picture-uri file://...`
  - Handle both `picture-uri` and `picture-uri-dark`
  - Handle multi-monitor: GNOME sets a single spanning image; composite to one file
- [ ] `SwayBackend`: detect `swww` or `swaybg` in PATH, prefer `swww` for smooth transitions
- [ ] `HyprlandBackend`: `hyprctl hyprpaper wallpaper "monitor,path"`
- [ ] `FehBackend`: `feh --bg-fill` for X11 fallback
- [ ] `CustomBackend`: user-supplied command from config with `{image_N}` substitution
- [ ] `superpanels profile apply/list/delete` CLI commands
- [ ] Slideshow: `RepeatedTimer` equivalent using `std::thread` + `Duration`

---

## Phase 3 — Tauri shell

**Goal:** `superpanels gui` opens a window. The tray icon works. Basic profile switching from tray is functional.

**Milestone:** Daily driveable as a tray app without touching the CLI.

- [ ] Scaffold Tauri v2 project in `src-tauri/`
- [ ] Register all `#[tauri::command]` wrappers (thin — just call `lib.rs`)
- [ ] System tray icon (SVG, light/dark adaptive)
- [ ] Tray right-click menu: profile list, next wallpaper, quit
- [ ] Main window open/close on tray left-click
- [ ] Basic Svelte UI scaffold in `ui/`
- [ ] `detect_monitors` IPC → render monitor boxes in a canvas (even if ugly)
- [ ] `apply_profile` IPC wired to apply button
- [ ] `autostart`: write `~/.config/autostart/superpanels.desktop` on demand

---

## Phase 4 — Polished GUI

**Goal:** The UI looks genuinely good. The interactive canvas is the standout feature — someone understands the whole app just by looking at it.

**Milestone:** Screenshots worth sharing.

### Interactive monitor preview canvas (`MonitorCanvas.svelte`)

The canvas is the heart of the UI. It renders the monitor layout to scale with the wallpaper image visible behind it, and lets the user compose the shot before applying.

**Rendering pipeline (each frame):**
1. Draw the wallpaper image scaled and positioned across the full canvas
2. Draw a dark semi-transparent overlay across the entire canvas
3. Use `destination-out` composite operation to punch a hole through the overlay for each monitor rectangle — the monitors appear as lit windows revealing the image beneath
4. Draw bezel gap bars (solid dark rectangles between monitors) on top
5. Draw monitor outlines (thin light border) on top

This gives the effect of the image being sliced by the monitor shapes — exactly the real-world result visualised in the UI.

**Accuracy:**
- [ ] Monitors rendered at correct relative physical sizes (mm dimensions, not just pixels) — a 27" 4K and a 24" 1080p look visibly different
- [ ] Correct relative positions — if monitor 2 sits 5cm lower than monitor 1, it renders that way
- [ ] Portrait monitors rendered at correct orientation (rotated rectangle)
- [ ] Bezel bars sized proportionally to the configured mm gap

**Interactivity:**
- [ ] Drag image horizontally (and vertically for stacked layouts) to reposition the composition
- [ ] On drag: fire `preview_crop(image_path, offset, bezels)` IPC call → Rust returns `Vec<CropSpec>` → canvas redraws in the same frame
- [ ] Bezel sliders update canvas in real time — no Apply needed to see the effect
- [ ] Hover a monitor: highlight with a subtle glow, show tooltip with monitor name and the pixel range it will receive from the source image
- [ ] Click a monitor: show a popout with the exact crop that will be applied to that screen (actual image region, not the canvas approximation)

**Apply animation:**
- [ ] On Apply: the overlay fades out, then each monitor rectangle briefly flashes (simulating the screen refreshing) before the overlay returns
- [ ] Subtle, fast (< 400ms total) — confirms the action without being annoying

**Edge cases:**
- [ ] 3+ monitors in a row
- [ ] Mixed monitor sizes
- [ ] Single monitor (canvas still renders, just one rectangle)
- [ ] Very wide ultrawide monitor alongside a standard one
- [ ] 2×2 grid layout

### IPC for the canvas

```
preview_crop(image_path: String, offset_px: i32, bezels: BezelConfig) -> Vec<CropSpec>
```

Called on every drag tick and every bezel slider change. Rust does the math (fast — pure arithmetic), returns crop rectangles, Svelte draws them. No image processing happens during preview — only the crop coordinates are computed.

### Profile management
- [ ] Profile list with active indicator
- [ ] Create new profile (name input → clones defaults)
- [ ] Delete with confirmation
- [ ] Settings persist on change (no explicit Save button for most fields)

### Polish
- [ ] Tailwind + custom CSS design system (dark mode first, matches KDE dark theme)
- [ ] Transitions on wallpaper apply
- [ ] Empty state: canvas shows placeholder monitor outlines with a "drop an image here" prompt
- [ ] Keyboard shortcuts (Apply: Enter, new profile: Ctrl+N, reposition reset: R)
- [ ] Error toasts for backend failures with copy-able error text

---

## Phase 5 — Packaging and release

**Goal:** Anyone on Arch can install it without reading a README.

- [ ] PKGBUILD for CLI binary (AUR: `superpanels`)
- [ ] PKGBUILD for GUI (AUR: `superpanels-gui`)
- [ ] GitHub Actions: build + test on push, release binaries on tag
- [ ] `cargo publish` to crates.io
- [ ] `README.md`: install instructions, one screenshot of the GUI, one CLI example

---

## Where to start

**First file to write: `src/display/mod.rs`**

Get `kscreen-doctor -o` parsed into a `Vec<Monitor>` with a test. Everything else depends on knowing what monitors exist. Once that's solid, `layout.rs` (the bezel math) is pure arithmetic that can be developed and unit-tested without any system at all.

Order that minimises blocked time:
```
display.rs → layout.rs (unblocked, pure math)
             → image.rs (unblocked, pure image processing)
                       → backends/kde.rs → wiring in main.rs → working MVP
```

`layout.rs` and `image.rs` can be developed in parallel with display detection since they take simple structs as inputs.
