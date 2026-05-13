# Layout math

The canonical reference for how Superpanels maps an image across monitors. Code lives in [`crates/superpanels-core/src/layout.rs`](../../crates/superpanels-core/src/layout.rs); this doc explains the *why* and the invariants.

> Historically this doc was "bezel math". The current term in the UI and code is **monitor gap** — the space between two adjacent monitors, which includes the bezels *and* any physical air-gap between panels. The math doesn't care which it is.

## Principle

The image maps onto the **physical desktop plane in millimetres**, including the space occupied by monitor gaps. Each monitor's crop is non-overlapping; content that falls in the gap simply isn't drawn, but it *is* accounted for, so visual continuity is preserved across the seam.

Pixel-only thinking gives wrong results when monitors have different PPI or sit with non-zero gaps between them. Always reason in mm.

## Authored placements drive gaps

There is no separate "bezel" or "gap" config value. Every profile carries a `monitor_state: HashMap<String, MonitorPlacement>` keyed by `stable_id` (or `name` fallback). `MonitorPlacement { x_mm, y_mm }` is the top-left of that monitor on the physical canvas. The monitor gap between any two monitors falls out of the difference between their authored `(x_mm, y_mm)` rectangles.

Rotation is **not** authored — it's OS-reported via `Monitor.rotation` and applied during layout.

## Inputs

- `monitors: &[Monitor]` — detected layout. `Monitor.physical_size_mm: Option<(f64, f64)>` must be `Some` for every entry; missing values fail with `LayoutError::PhysicalSizeMissing`.
- `placements: &HashMap<String, MonitorPlacement, _>` — from the active profile's `monitor_state`.
- `image_size`, `image_rect_mm`, `fit_mode` — together control how the source rectangle is sized on the canvas.

## Algorithm

1. **Collect monitors.** Sort deterministically. Reject if any monitor has `physical_size_mm == None`.
2. **Apply rotation** to each monitor: a portrait 27" panel's effective width is its short side, its height its long side.
3. **Resolve the source rectangle on the canvas.**
   - When the profile pins `image_rect_mm: ImageRectMm`, use it directly.
   - Otherwise derive from `FitMode` and the monitors' bounding box.
4. **Pick reference PPI.** Default to `max(monitor.ppi)` so the densest panel paints native-resolution; user can override per-profile.
5. **For each monitor, compute `src_rect`** in source-image pixels:
   - `origin_mm = placement.(x_mm, y_mm)`
   - `size_mm = effective monitor size after rotation`
   - convert mm → reference-PPI px → source-image px (account for image scale)
6. **Clamp `src_rect`** to `[0, image_w] × [0, image_h]`. Anything outside the image becomes letterbox padding — the apply pipeline's `compose_on_black` paints those pixels black at the slice's `dst_offset` / outside its `slice_dst_size`. Drag offsets that push the source rectangle off-image are valid; there is no hard error for it.
7. **Resample** each monitor's crop to native resolution (post-rotation).
8. **Rotate** before save so the temp file is upright.
9. Hand each `(image_path, MonitorRef)` to the backend.

## Mixed PPI

Normalise to a reference PPI before computing crops so the image appears at the same physical scale on each screen. Default reference: the maximum PPI across all monitors. Per-profile override available.

## Mixed orientation

A portrait monitor contributes its rotated dimensions to the physical canvas. Internally, every dimension stored in `physical_size_mm` is **native orientation**; the layout module applies rotation when building the canvas. The crop rectangle is rotated by `Monitor.rotation` before save, so the temp file is always upright. The compositor sees a normal image; rotation is baked in.

> **KDE wallpaper plugin gotcha:** save files at the *canvas/logical* (post-rotation) dimensions, not native panel dimensions. KDE's framebuffer for the wallpaper plugin is already rotated. Pre-rotating produces letterbox bars. This was burned in once; don't burn it again.

## Edge cases the algorithm handles

- A single monitor (degenerate canvas — math is a no-op).
- 3+ monitors in a single row.
- 2×2 grid.
- Mixed sizes side-by-side (e.g. 34" ultrawide + 24" 1080p).
- One landscape + one portrait.
- Monitors with non-zero vertical offset that the desktop reports as the same row.
- HiDPI: a 2.0-scale 4K reports `1920×1080` logical px but `3840×2160` native; layout uses native.

## What the math deliberately does *not* do

- It doesn't try to hide content "behind" the gap by *omitting* gap pixels — that produces visible duplication at the seam. Crop at the gap boundary and skip the gap.
- It doesn't perspective-correct toed-in monitors.
- It doesn't try to be smart about subject framing.

## Property tests

The layout math is property-tested in `crates/superpanels-core/src/layout.rs` (and `layout/algorithm.rs`) with `proptest`. Invariants we hold:

- Crop count equals monitor count.
- Crops never overlap in source-image pixels.
- Total `src_rect` area never exceeds the image's pixel area.

When `proptest` finds a failure it minimises automatically and saves the case to `tests/proptest-regressions/` so it gets re-run forever.
