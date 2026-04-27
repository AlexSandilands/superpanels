# 3. Core concepts

## 3.1 Monitor

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

`MonitorId` is **runtime-only**: it's assigned during detection, used to address monitors during a single apply, and never persisted. For data that must survive reboots and dock re-plugs (per-monitor profile assignments, bezel overrides, physical-size config) use `MonitorRef` (§6.4) instead.

`physical_size_mm` is **user-supplied**, not detected. The reason: `kscreen-doctor` (and most compositor CLIs) do not expose physical mm; reading EDID from sysfs is a workable fallback but EDID values are sometimes wrong and irrelevant on virtual displays. Since the user is already configuring bezel widths in mm, asking them for monitor dimensions in mm is a small additional step (and the GUI's first-run flow can prefill from "diagonal + aspect" — see §12.6). When `physical_size_mm` is `None`, bezel math returns `LayoutError::PhysicalSizeMissing` listing the monitors that need configuration.

`rotation` matters for the layout math: a portrait monitor's *physical* width is its (rotated) short side and its height is its long side. Everything in `physical_size_mm` is recorded in native orientation; the layout module applies the rotation when building the desktop's physical canvas.

## 3.2 BezelConfig

Physical gap sizes between adjacent screens, specified in millimetres.

```rust
struct BezelConfig {
    horizontal_mm: f32,                                // uniform gap between any pair of horizontally adjacent monitors
    vertical_mm: f32,                                  // uniform gap between any pair of vertically adjacent monitors
    overrides: HashMap<(MonitorRef, MonitorRef), f32>, // optional per-pair override; key is sorted-pair
}
```

Default is uniform `horizontal_mm` / `vertical_mm`, which covers ~90% of real setups. Overrides exist for the rare case where bezel widths differ — e.g., a thin-bezel ultrawide between two thick-bezel old IPS panels. The override key is normalised so `(a, b)` and `(b, a)` collapse to the same entry; see `MonitorRef` (§6.4) for why we key on `MonitorRef`, not `MonitorId`.

## 3.3 CropSpec

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

## 3.4 Profile

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

## 3.5 ImageSet

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
