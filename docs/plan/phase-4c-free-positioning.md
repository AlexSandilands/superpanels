# Phase 4c — Free-positioning canvas

**Goal.** Switch the canvas mental model from *"image only visible through monitor cutouts"* to *"image floats freely on the canvas; monitors are crop windows that hover over it."* Add drag-to-pan + drag-corner-to-resize so the user can place and scale the source rectangle anywhere on the desktop plane, with monitors letterboxing whatever they don't cover with black.

This phase exists because the original SPEC §12.3 five-layer model is conceptually pure (the bezel really *is* the wall behind the monitors) but creates two practical problems:

1. **Mixed orientations are guesswork.** A portrait + landscape pair shares no common axis to scale across; the user has no way to see what they're cropping out.
2. **The drag-then-Apply path crashes.** `compute_crop_specs_with_offset` (`crates/superpanels-core/src/layout/algorithm.rs:285-310`) computes `src_left = src_origin + (canvas_x - offset) * scale`, then materialises a `Rect { x: u32, w: u32, ... }` (`layout.rs:23-29`). Any drag big enough to push the source-pixel origin negative explodes the cast and surfaces as a `LayoutError` toast on Apply. Free positioning fixes this by design: the crop algorithm clamps `src_rect` to the image and emits letterbox metadata for the apply layer to fill with black.

**Definition of done.**
- [x] Image is visible across the full canvas (not only inside monitor cutouts). Monitors render as bordered overlays on top.
- [x] Drag-to-pan works at ≥ 60 fps and Apply succeeds for *any* offset, including offsets that put part of the source rectangle off-image.
- [x] Drag-corner-to-resize works; aspect-ratio lock + free modes both supported.
- [x] Per-monitor regions not covered by the source rectangle are written as black pixels by the apply pipeline (no `LayoutError`, no daemon toast).
- [x] Mixed portrait + landscape: the user can scale the source so it covers both orientations consistently, and the visual preview matches the applied wallpaper.
- [x] Old span profiles (no `image_size_mm` in TOML) load with the legacy "auto-fit per FitMode" behaviour preserved — no migration required.

## 4c.1 Profile schema — add image transform

Add a third field next to `offset` on `SpanProfile`:

```rust
pub struct SpanProfile {
    pub source: SpanSource,
    #[serde(default)]
    pub fit: FitMode,
    /// Image-position offset in canvas px (`SPEC.md` §8.3).
    #[serde(default)]
    pub offset: [i32; 2],
    /// Optional explicit image size in canvas px. `None` = use FitMode to
    /// derive from the source dims (the legacy behaviour). `Some(w, h)` =
    /// user has positioned the image freely; FitMode is ignored on Apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_size_px: Option<[u32; 2]>,
}
```

- [x] Add the field with `#[serde(default, skip_serializing_if = ...)]` so existing TOML round-trips unchanged.
- [x] `cargo test -p superpanels-core` covers: `None` round-trips through TOML; `Some([w, h])` round-trips; an existing `[[profile]]` block without the field deserialises with `image_size_px: None`.
- [x] Mirror the field in `ui/src/lib/types/profile.ts`. Frontend treats `null` as "use FitMode" and a tuple as "explicit transform."
- [x] `ui/src/lib/stores/profile.svelte.ts::patchDraft` is unchanged structurally — the new field is just another mutable slot.

**Why this shape, not a `scale: f32`:** scale forces a base-size choice (image native? canvas? FitMode result?) that has to stay consistent across the canvas, the Rust crop, and the daemon. Storing the *resulting* rectangle in canvas px is unambiguous: canvas + Rust agree on what the user picked. Cost: the value depends on `compute_canvas_pixels` output, so users that hand-edit TOML have to do the math. That's an acceptable trade because the GUI is the only sensible authoring surface.

## 4c.2 Crop algorithm — clamp + letterbox

Currently `SrcMapping::monitor_to_src_rect` (`crates/superpanels-core/src/layout/algorithm.rs:280`) produces an unclamped src rectangle and trips on the `u32` cast when it goes negative.

- [x] Change the return type from `Result<Rect, LayoutError>` to `Result<CropSliceSpec, LayoutError>` where:
  ```rust
  pub struct CropSliceSpec {
      /// Clamped src rect inside the source image. Always inside [0, image_w] × [0, image_h].
      pub src_rect: Rect,
      /// Offset in dst pixels where the cropped region paints. (0, 0) = covers
      /// the whole monitor; non-zero = letterbox padding on the top-left.
      pub dst_offset: (u32, u32),
      /// Dst region the src_rect fills. Equal to monitor dst_size when fully
      /// covered; smaller when the source rect doesn't reach the monitor edge.
      pub dst_size: (u32, u32),
  }
  ```
- [x] `CropSpec` (the existing top-level type) embeds `CropSliceSpec` so the apply pipeline knows where to paint and what to leave black. Existing fields `monitor_id`, `rotation`, `fit` stay.
- [x] `compute_crop_specs_with_offset` honours `image_size_px`: when `Some`, the source rectangle on the canvas is `(offset.x, offset.y, image_size_px.0, image_size_px.1)` regardless of FitMode; when `None`, the existing FitMode-driven placement is used.
- [x] **Tests** (proptest where useful):
  - [ ] Drag offset large enough to put `src_rect.x` past the image right edge → `src_rect` is empty (zero-area `Rect`), `dst_offset == (0, 0)`, `dst_size == (0, 0)` (nothing to paint, monitor goes fully black).
  - [ ] Drag offset that partially covers a monitor → `src_rect` is a valid sub-rect of the image, `dst_offset` and `dst_size` describe the covered portion of the monitor.
  - [ ] No-offset, default FitMode → output matches today's `Rect` byte-for-byte (regression).
  - [ ] `image_size_px = Some([2*src_w, 2*src_h])` (zoomed 2×) → src_rect of each monitor is half the size of the no-zoom case.
  - [ ] Property: `dst_offset.x + dst_size.0 ≤ monitor.dst_size.0` and `dst_offset.y + dst_size.1 ≤ monitor.dst_size.1` (letterbox stays inside monitor).
  - [ ] Property: `src_rect` is always inside `(0, 0, image_w, image_h)` (no negative indices, no overflow).

## 4c.3 Apply pipeline — pad uncovered regions with black

The single-monitor apply path (`crates/superpanels-cli/src/profile_cmd.rs::run_span_apply`, `crates/superpanels-daemon/src/apply.rs::run_span_apply`) currently feeds `crop` → `scale_to_fit` → `rotate` → `save_temp`. With letterboxing, the slice doesn't fill the monitor; we have to compose it onto a black canvas first.

- [x] Add `superpanels_core::image::compose_on_black(slice: DynamicImage, dst_size: (u32, u32), dst_offset: (u32, u32)) -> DynamicImage` (or extend `crop`/`scale_to_fit` rather than introducing a new function — pick whichever keeps `crates/superpanels-core/src/image.rs` under the line cap).
- [x] Wire it into both apply paths between `crop` and `rotate` so the temp file is always full monitor resolution post-rotation.
- [x] Test: an apply with a deliberately small `image_size_px` produces a temp file with black borders around the actual image content; size matches monitor `dst_size`.
- [x] Test: existing offset-zero / FitMode::Fill apply produces identical bytes to today's path (no spurious recompositing).

## 4c.4 Canvas rendering — flip the layer order

Currently `ui/src/lib/canvas/draw.ts::drawCanvasLayers` paints:

1. background → 2. wallpaper clipped to layout bbox → 3. dark overlay with monitor cutouts → 4. bezels → 5. outlines.

Replace with the free-floating model:

1. **Background** (canvas-wide, slate dark).
2. **Image rectangle** at `(offset.x, offset.y, image_size_px.0, image_size_px.1)` — visible across the entire canvas, no layout-bbox clip.
3. **Per-monitor backing**: black fill the size and position of each monitor rect. (This is what the user will *actually* see on each panel — the black is the letterbox.)
4. **Image inside monitors**: re-draw the image clipped to the union of monitor rects only. Painting black-then-image gives the same result as letterboxing on the apply side, and the canvas mirrors what Apply will produce.
5. **Bezels** between monitors (unchanged).
6. **Monitor chrome**: outlines (heavier than today since the monitors are now visually distinct from the rest of the image), labels, hover glow.
7. **Resize handles** at the four corners of the image rectangle (only when the image is the active drag target — see §4c.5).

Steps 2 and 4 use the same `ctx.drawImage` call but with different clip paths. Pixel-equivalent to "draw image once, paint dark overlay with monitor cutouts" but reads more clearly and lets us tint the outside-monitor region differently if we want (e.g. desaturate or dim the off-monitor preview to make the cropping obvious — see §4c.7).

- [x] Update `draw.ts` to the new layer order. Keep the `serialiseLayout` snapshot helper unchanged so 4d's regression test still works.
- [x] Update `ui/src/components/MonitorPopout.svelte`'s slice math: the source rectangle is now `(offset, image_size_px)` directly instead of `placeImage(fit, ...)`. Simpler.
- [x] Test: a saved `image_size_px = None` profile renders identically to today (visual regression via the 4d snapshot harness).

## 4c.5 Canvas interactivity — pan + corner-resize

The pointer-handler block in `MonitorCanvas.svelte` (lines ~229-301) needs three modes:

- **Pan**: pointer down on the image body → drag updates `offset`. Existing behaviour, unchanged math.
- **Resize**: pointer down on a corner handle → drag updates `image_size_px` and (for corners that aren't the bottom-right) `offset` so the opposite corner stays anchored.
- **Hit-test fallthrough**: pointer down on a monitor rect (when not on the image) opens the popout (existing behaviour).

Hit-priority on pointer-down: corner handles → image body → monitor rect.

- [x] Four 12×12 px corner handles drawn at the image rect corners. Cursor style per corner (`nwse-resize`, `nesw-resize`).
- [x] Drag from a corner mutates `image_size_px` and (when the user holds Shift, or by default — pick one) preserves the source aspect ratio.
- [x] Free-resize mode (no aspect lock) for users who want to stretch — surfaces as a small "lock aspect" toggle next to the FitMode buttons. Default is locked.
- [x] Snap-to-cover button: a single "Cover all monitors" action (next to the fit buttons) that sets `offset` and `image_size_px` so the smallest scale that fully covers the union of monitor rects is applied. Saves the user from manually scaling for the portrait+landscape case.
- [x] `R` key behaviour: today resets offset to `[0, 0]`. New behaviour: resets *both* `offset = [0, 0]` and `image_size_px = None` (back to FitMode-driven default).
- [x] Pointer math respects the `offsetScale` factor (`mmToPx / coreMmToPx`) so the persisted core-canvas-pixel values match what the Rust crop algorithm expects.
- [x] Drag commit batches `offset` + `image_size_px` into a single `patchDraft` call so the editor's `unsaved` flag fires once per gesture, not twice.

## 4c.6 ProfileEditor — show the transform numerically

The user can now position the image precisely; expose the numbers so they can fine-tune.

- [x] In `ProfileEditor.svelte`, when `body.type === 'span'` and `body.image_size_px !== null`, show a small read-only panel with the current `offset` (px) and `image_size_px` (px). A "Reset transform" button clears both back to FitMode behaviour.
- [x] FitMode picker is greyed out (with a tooltip) when `image_size_px` is set, since the explicit transform supersedes it. Picking a FitMode while greyed clears `image_size_px` and re-derives.
- [x] Test: snapshotting a profile through Save → Reload via `profileStore.refresh()` round-trips both fields losslessly.

## 4c.7 Off-monitor dimming (visual cue)

To make it obvious which part of the image is going to land on the wall:

- [x] In `draw.ts` step 2 (image rectangle), draw the off-monitor portion of the image at reduced alpha (e.g. 0.4) and the on-monitor portion at full alpha. Implementation: draw at full alpha into a temp canvas, then alpha-fade the regions outside `layout.monitors`. Or draw twice — once at 0.4 across the whole image bbox, once at 1.0 with monitor-rect clip.
- [x] Toggle (`D` key, or a small icon button in the canvas chrome) to disable the dimming if it's distracting.
- [x] Respect `prefers-reduced-motion` only insofar as no animation is involved — the dim is a static effect, fine to keep on.

## 4c.8 SPEC updates

The new model needs to be documented before code lands so the spec stays the source of truth:

- [x] Update `docs/spec/12-gui.md` §12.3:
  - Replace the five-layer compositing description with the seven-layer free-positioning version (mirror §4c.4 here).
  - Replace the "drag the image to reposition" line with the pan + resize + snap-to-cover summary.
  - Add a sentence on the `image_size_px: Option<...>` field and what `None` means.
  - Update §12.5 keyboard shortcuts: `R` now resets transform (offset + image size), `D` toggles dim.
- [x] Update `docs/spec/04-bezel-math.md` §4.6 ("What the math deliberately does *not* do"):
  - Add a note that the math now supports user-supplied `image_size_px`, which overrides FitMode.
  - Add a note that monitor regions outside the source rectangle are letterboxed with black on Apply.
- [x] Update `docs/spec/08-image-processing.md` to reference the `compose_on_black` step (or whatever shape §4c.3 lands on) in the apply pipeline.

## 4c.9 Migration / compatibility

- [x] Existing TOML profiles without `image_size_px` continue to apply correctly — verified by an integration test that loads a Phase 4a config and runs `apply_profile`.
- [x] `superpanels set` CLI: no behaviour change. The CLI doesn't expose `image_size_px` because positioning is a GUI affordance; CLI users get the FitMode-driven default.
- [x] The new `CropSliceSpec` shape is internal; `preview_crop`'s IPC return type still serialises a `Vec<CropSpec>`, with `CropSpec` extended (additive fields).

**Risks for this phase.**
- **Schema migration.** Adding `image_size_px` is a `serde(default)` additive change — existing TOML reads cleanly. Verified by tests in §4c.1.
- **Pixel-perfect parity for legacy profiles.** §4c.4's "draw image, then dark overlay with cutouts" model is *not* literally what the new code does, but it should be visually indistinguishable for `image_size_px = None`. The visual-regression snapshot from 4d will guard this, but it doesn't exist yet — until it does, eyeball-check that a saved Phase-4a profile renders the same.
- **Resize gesture vs. pan gesture.** Hit-priority must be deterministic; corner handles win even when they overlap a monitor rect. Add a hit-test test that exercises corner-on-monitor overlaps.
- **Apply-time recomposition cost.** `compose_on_black` runs once per monitor on Apply. For a 4K monitor that's ~30 MiB of pixels — milliseconds, but worth verifying it doesn't push past the SPEC §19 apply budget.

## 4c.10 Performance

- [x] Canvas redraw must still hit the SPEC §19 budget (< 8 ms / ≥ 120 fps median) with the extra image draw + dim layer. The `localStorage.superpanels.bench` hook from Phase 4a still works; re-capture the median + p95 after §4c.4 lands.
- [x] Apply path: measure end-to-end with letterboxing on a 3-monitor config (one zoomed-out so two monitors are partially uncovered). Should stay under SPEC §19's apply budget. If `compose_on_black` is the bottleneck, the optimisation is to fuse it with the existing `scale_to_fit` step rather than copy-once-pad-once.
