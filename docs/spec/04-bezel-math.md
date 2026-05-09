# 4. The bezel & layout math

The principle: **the image maps to the *physical* screen plane, including the space occupied by bezels.** The crops handed to each monitor are non-overlapping; the content that "falls" in the bezel gap simply isn't drawn, but it is *accounted for* in the layout, so visual continuity is preserved across the bezel.

**Gaps are derived from authored placements.** As of v0.8 there is no separate `BezelConfig` — each profile's `monitor_state: HashMap<StableId, MonitorPlacement>` carries the per-monitor `(x_mm, y_mm)` directly, and the bezel/air-gap between any two monitors falls out of the difference between their authored placements. This eliminates the double-bookkeeping where the same physical gap was authored both as a bezel value and as a position difference.

## 4.1 Worked example — two identical monitors, uniform gap

```
Physical layout (mm):   [==monitor 1==][bezel|bezel][==monitor 2==]
                         <--- W1 mm ---> <-- G mm --> <--- W2 mm --->

Total physical width = W1 + G + W2 (mm)
Image pixels per mm  = image_width_px / total_physical_width_mm

Monitor 1 crop:       x = 0,                     w = W1_mm * px_per_mm
Monitor 2 crop:       x = (W1 + G) * px_per_mm,  w = W2_mm * px_per_mm
```

## 4.2 Mixed PPI

When monitors have different pixel densities, normalise to a reference PPI before computing crops, so the image appears at the same physical scale on each screen. Reference PPI is the maximum across all monitors by default; user can override per-profile.

## 4.3 Mixed orientation

A portrait monitor contributes its rotated dimensions to the physical canvas: a 27" 16:9 panel in portrait orientation becomes ~336 mm wide × 597 mm tall. The crop handler rotates the cropped pixel rectangle by the monitor's `rotation` before saving, so the temp file is always written in the orientation the monitor will display it. The compositor sees a normal upright image; rotation is *baked in* during processing.

## 4.4 General algorithm

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

## 4.5 Edge cases the layout module must handle

- A single monitor (degenerate canvas — bezel math is a no-op).
- 3+ monitors in a single row.
- 2×2 grid (Sway/Hyprland tiling-WM users do this).
- Mixed sizes side-by-side (e.g. 34" ultrawide + 24" 1080p).
- One landscape + one portrait (the headline rotation case).
- Monitors with non-zero `position` offset that the desktop doesn't expose as a row (e.g. one monitor 200 px lower than the other).
- HiDPI + scale factor (a 2.0-scale 4K reports 1920×1080 logical px but 3840×2160 native).

## 4.6 What the math deliberately does *not* do

- It doesn't try to hide image content "behind" the bezel by *omitting* the bezel pixels from the source — that produces visible duplication at the seam. It crops at the bezel boundary and skips the gap.
- It doesn't perspective-correct angled monitors. If the user toes their monitors in, that's their problem (see roadmap).
- It doesn't try to be smart about subject framing (e.g. "keep the face on the centre monitor"). v2.

**Phase 4c additions:**

- The crop algorithm now honours an optional user-supplied `image_size_px` per profile. When set, the source rectangle on the canvas is `(offset.x, offset.y, image_size_px.0, image_size_px.1)` regardless of `FitMode` — the GUI's free transform overrides the FitMode-driven placement.
- Per-monitor `src_rect` is always **clamped** to `[0, image_w] × [0, image_h]`. Anything outside the image becomes letterbox padding: the apply pipeline's `compose_on_black` step paints those pixels black at the slice's `dst_offset` / outside its `slice_dst_size`. There is no longer a hard `LayoutError` for drag offsets that push the source rectangle off-image.
