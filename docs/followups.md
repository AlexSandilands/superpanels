# Follow-ups

Loose ends that aren't blocking, but should be revisited. Add an entry when
deferring a workaround or "we'll do this later" item; remove it when done.

## WebKitGTK DMABUF renderer workaround

`WEBKIT_DISABLE_DMABUF_RENDERER=1` is set in three places to dodge a WebKitGTK
crash (`Gdk-Message: Error 71 (Protocol error)`) on Wayland under common
NVIDIA / Mesa + KDE Plasma 6 stacks:

- `.cargo/config.toml` — for `cargo run` / `cargo test`.
- `justfile` (`gui` recipe) — for direct release-binary invocation.
- `crates/superpanels-gui/src/autostart.rs` (`DESKTOP_BODY`) — for the
  installed `~/.config/autostart/superpanels.desktop` entry.

**Revisit when:** WebKitGTK ships a fix for the DMABUF renderer crash on the
affected stacks (track upstream WebKit / `webkit2gtk` Arch package).

**Action when fixed:** drop the env var from all three sites and the
`set_enabled_at_true_writes_desktop_file` /
`desktop_body_includes_webkit_dmabuf_workaround` test assertions in
`autostart.rs`.

## Fix transforms/cropping from canvas
  - Align with the preview in the profile manager

## Slideshows
  - Allow different canvas settings per image from the slideshow

Design notes (feasibility reviewed 2026-06-10, after the slideshow feature
landed):

- **Model**: per-image override = the canvas snapshot a profile already
  persists — `monitor_state` (placements, i.e. where gaps live) +
  `image_rect_mm`. Store as a sparse map inside `SpanSource::Slideshow`
  (`overrides: HashMap<PathBuf, …>`, entries only for hand-tuned images) so
  it rides the existing `update_profile_source` pipeline: shared
  `Config::set_span_source` core helper, validated `save_to`, optimistic +
  serialized persistence in the GUI. Optional `#[serde(default)]` field —
  backward-compatible on disk and wire. Cap the map size in
  `config/validate.rs` (same spirit as `MAX_SLIDESHOW_IMAGES`).
- **Apply**: every slideshow apply funnels through `run_span_apply`; resolve
  "effective profile for this path" (clone with override's placements + rect
  patched in) at that one choke point. Layout math is unchanged — it already
  computes crops from whatever placements it's handed. Overrides resolve
  daemon-side so they work with the GUI closed.
- **UX**: "save current canvas for this image" — pause on an image, drag gaps
  and rect, save-for-this-image. The **canvas follows overrides live**: when
  the slideshow advances onto an overridden image, the preview re-gaps and
  re-places to match (consistent with `liveSlideshowPath` mirroring today),
  not only in an explicit edit mode.
- **Dirty tracking**: `canvasOverridesDirty` + `imageTransformDirty` already
  diff placements/transform against a baseline; the work is swapping the
  baseline to the live image's override when one exists. This is entangled
  with the preemption-sentinel machinery (see entry below) — fixing that
  first de-risks this feature.
- **Topology repair**: `monitor_state` is keyed by `stable_id`; the repair
  flow must remap (or at minimum invalidate, with a toast) every per-image
  override map, not just the profile-level one. In scope from day one or
  overrides silently break after a monitor swap.
- **Known limitation**: overrides keyed by absolute path — a rename/move
  drops the tweak. Library grid should badge "has custom placement" and
  offer a reset; orphaned entries need GC or tolerance.

## Preemption sentinel fires on user-initiated switches

`switchAndApply` claims the *new* profile name as the preemption sentinel
before `activeName` has refreshed, so the schedule-preemption `$effect` in
`App.svelte` briefly sees `sentinel !== activeName` and treats the user's own
switch as an external change — it can re-select the old profile for a tick
(and could surface a spurious "Schedule switched" toast when the canvas is
dirty). The library modal now derives its slideshow-edit mode reactively to
ride out the flip-flop, but the sentinel handshake itself should compare
against "switch in flight" state instead of raw `activeName`.

## Monitor gap not loaded on app start for profile

## Repair different topology

## Lag when opening profile switcher

## Per Monitor Wallpapers

## Set icon in taskbar

## Remove everything related to primary monitor
