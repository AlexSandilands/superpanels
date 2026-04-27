# Phase 4a — Canvas interaction

**Goal.** The headline canvas works. Drag-to-offset, live bezel sliders, accurate physical-mm rendering. No library work yet — the canvas is the unit Phase 4 was over-stuffed for, and it deserves its own phase.

**Definition of done.**
- [ ] Drag the image in the canvas; the crop updates at ≥ 60 fps; releasing applies the new offset to the active profile.
- [ ] Bezel sliders update the canvas in real time.
- [ ] Drag-and-drop image into the window adds it to the active profile.
- [ ] Three clean screenshots: empty state, single-monitor canvas, three-monitor canvas.

## 4a.1 Canvas — rendering pipeline
- [ ] Five-layer compositing per SPEC §12.3:
  - [ ] Wallpaper image layer (using a thumbnail of the source, never the full image during interaction).
  - [ ] Dark overlay.
  - [ ] `destination-out` cut-outs for monitors.
  - [ ] Bezel bars.
  - [ ] Outlines + labels.
- [ ] Renders at canvas resolution, redraws via `requestAnimationFrame`.
- [ ] Monitor labels with name, resolution, and physical size.

## 4a.2 Canvas — accuracy
- [ ] Monitors at correct relative *physical* sizes (mm).
- [ ] Correct relative positions (a monitor lower in `position.y` renders lower).
- [ ] Portrait monitors as rotated rectangles.
- [ ] Bezel bars proportional to mm gap.
- [ ] Visual regression tested by capturing the canvas state to JSON (positions/sizes); diff is meaningful and reviewable.

## 4a.3 Canvas — interactivity
- [ ] Drag offset: pointer events → IPC `preview_crop` → redraw.
- [ ] Bezel sliders: live update; crops + bar widths update on every `input` event.
- [ ] Hover monitor: glow + tooltip with src pixel range.
- [ ] Click monitor: side popout with the exact crop preview (uses the thumbnail to show the slice).
- [ ] `R` key resets offset.
- [ ] Wheel/pinch: zoom 0.5×–2.0× for inspection (does not affect applied result).
- [ ] Apply animation < 400 ms, fade overlay → per-monitor flash → fade in. Replaced by instant transition under `prefers-reduced-motion`.

## 4a.4 Profile editor (canvas-adjacent)
- [ ] Inline form on the right side: image source picker, body type (Span / PerMonitor), fit, bezels, slideshow config.
- [ ] Per-monitor pin UI for `PerMonitor` body (drop image onto monitor in canvas).
- [ ] Schedule editor: visual chooser for daily-time / sunset-offset / cron.
- [ ] Save button or autosave (autosave for non-destructive fields like fit; explicit save for destructive ones like image source).

**Risks for this phase.**
- The canvas's drag interaction sending an IPC roundtrip per frame may bottleneck on Tauri serialisation. If profiling shows > 5 ms per call, port the crop math to TypeScript so it runs in-process and call IPC only on release.
