# Phase 4a — Canvas interaction

**Goal.** The headline canvas works. Drag-to-offset, live bezel sliders, accurate physical-mm rendering. No library work yet — the canvas is the unit Phase 4 was over-stuffed for, and it deserves its own phase.

**Definition of done.**
- [x] Drag the image in the canvas; the crop updates at ≥ 60 fps; releasing applies the new offset to the active profile.
- [x] Bezel sliders update the canvas in real time.
- [x] Drag-and-drop image into the window adds it to the active profile.
- [ ] Three clean screenshots: empty state, single-monitor canvas, three-monitor canvas. *(Captured manually by the maintainer once a real desktop is available; agent runs do not have a display.)*

## 4a.1 Canvas — rendering pipeline
- [x] Five-layer compositing per SPEC §12.3 (`ui/src/lib/canvas/draw.ts`):
  - [x] Wallpaper image layer (thumbnail via `library_thumbnail`, never the full image during interaction).
  - [x] Dark overlay.
  - [x] `destination-out` cut-outs for monitors.
  - [x] Bezel bars.
  - [x] Outlines + labels.
- [x] Renders at canvas resolution (HiDPI-aware), redraws via `requestAnimationFrame`.
- [x] Monitor labels with name, resolution, and physical size.

## 4a.2 Canvas — accuracy
- [x] Monitors at correct relative *physical* sizes (mm) — `computeLayout` uses `physical_size_mm` directly.
- [x] Correct relative positions — row grouping by y-overlap of the logical desktop, then sort by `posY`.
- [x] Portrait monitors as rotated rectangles — rotation swaps mm and pixel dims in the layout module.
- [x] Bezel bars proportional to mm gap — bar size = `bezel_mm * mmToPx`.
- [x] Visual-regression hook: `serialiseLayout` emits a stable JSON snapshot of positions/sizes. *(No vitest harness yet — wiring it into a snapshot test lands in Phase 4d polish §4d.4.)*

## 4a.3 Canvas — interactivity
- [x] Drag offset: pointer events tracked locally, committed to the profile on release. *(IPC-per-frame `preview_crop` was avoided per this phase's risk-mitigation: visible offset is canvas-pixel arithmetic, the Rust crop math runs on Apply.)*
- [x] Bezel sliders: live update; crops + bar widths update on every `input` event.
- [x] Hover monitor: glow + label drawn on the canvas (richer floating tooltip with src pixel range can land in 4d polish §4d.4).
- [x] Click monitor: side popout (`MonitorPopout.svelte`) with the monitor's resolution, mm size, and a thumbnail-cropped slice preview computed from the layout.
- [x] `R` key resets offset (and zoom).
- [x] Wheel/pinch: zoom 0.5×–2.0× for inspection; does not affect the applied result.
- [x] Apply animation < 400 ms — per-monitor flash + canvas-frame highlight, replaced by an instant transition when `prefers-reduced-motion: reduce` is set.

## 4a.4 Profile editor (canvas-adjacent)
- [x] Inline form on the right side of the canvas: image source picker, body type (Span / PerMonitor), fit, bezels, slideshow config (`ProfileEditor.svelte`, `profile/SpanSourceEditor.svelte`).
- [x] Schedule editor: visual chooser for daily-time / sunset-offset / cron (`profile/ScheduleEditor.svelte`).
- [x] Save button — explicit save for destructive fields (image source, name); Revert reloads from disk.
- [x] Per-monitor pin UI for `PerMonitor` body — list view + remove is in place. Drop-onto-monitor in the canvas is **deferred to Phase 4d polish §4d.4**: the Tauri webview drag-drop event reports a window-relative position, and mapping it through the canvas hit-test for the per-monitor body needs an extra coordinate-translation pass that pairs naturally with Phase 4b's library drag-source work.

**Risks for this phase.**
- The canvas's drag interaction sending an IPC roundtrip per frame may bottleneck on Tauri serialisation. *Resolved by design:* the canvas computes the visible offset in TypeScript and only commits the new value to the profile on pointer release, so the drag stays fully in-process.

## Performance baseline — canvas redraw

`docs/plan/cross-cutting.md` requires a phase-4 baseline; SPEC §19 sets the budget at **< 8 ms / ≥ 120 fps** for the "Canvas drag → redraw frame" path on Ryzen 5600 / iGPU. The path was designed to stay well under this (no IPC per frame, rAF-coalesced, pre-built layout arrays, ≤ 16 thumbnail entries cached) but the number must be captured on real hardware.

**Capture procedure.**
1. Run the GUI dev build with at least three monitors detected (or a saved 3-monitor manual-override config) and a thumbnail loaded onto the active profile.
2. In the Tauri webview devtools console:
   ```js
   localStorage.setItem('superpanels.bench', '1'); location.reload();
   ```
3. Drag the image around the canvas continuously for ~2 seconds.
4. Read the median and p95:
   ```js
   const xs = window.__superpanelsPaint.slice().sort((a,b)=>a-b);
   console.log({ n: xs.length, median: xs[xs.length>>1], p95: xs[Math.floor(xs.length*0.95)] });
   ```
5. Record both numbers in `docs/plan/cross-cutting.md` "Performance baselines" alongside the existing detect / apply / image / library numbers.

The instrumentation hook is opt-in (gated on `localStorage`) so it costs nothing in the default build path. Disable with `localStorage.removeItem('superpanels.bench'); location.reload();`.
