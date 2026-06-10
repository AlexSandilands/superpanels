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

When the topology-repair flow lands, it must remap (or at minimum
invalidate, with a toast) the per-image `overrides` maps inside
`SpanSource::Slideshow`, not just the profile-level `monitor_state` — both
are keyed by `stable_id`, so a monitor swap silently breaks hand-tuned
slideshow images otherwise.

## Lag when opening profile switcher

## Per Monitor Wallpapers

## Set icon in taskbar

## Remove everything related to primary monitor
