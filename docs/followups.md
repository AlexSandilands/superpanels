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

## Cover `debounce_and_redetect` with a hermetic test

`crates/superpanels-daemon/src/display_watch.rs` `debounce_and_redetect`
collapses bursts of kscreen signals into a single re-detect (~250 ms
window). It currently has no unit test because zbus's `MessageStream` is
concrete and ties into a real `Connection`. Worth introducing a small
trait abstraction (e.g. `trait SignalSource: Stream<Item = ...>`) so a
test can drive coalescing under `tokio::time::pause()` and assert that
N signals within DEBOUNCE produce exactly one publish.

## Pin a real D-Bus signal for rotation push on Plasma 6

`display_watch.rs` subscribes to `org.kde.KScreen` as a best-effort push
path, but on Plasma 6 Wayland the kscreen kded module is often unloaded
and the signal doesn't fire. Manual re-detect (Settings > Monitors, F5)
is the working fallback, but a real push signal would feel snappier.

**Action:** run `dbus-monitor --session "type='signal'"` while rotating
a display in System Settings, identify what actually fires (likely a
KWin or kded signal we haven't pinned), and update `build_match_rule`
to target it.

## Fix transforms/cropping from canvas
  - Align with the preview in the profile manager

## Slideshows

## Monitor gap not loaded on app start for profile

## Repair different topology

## Lag when opening profile switcher

## Per Monitor Wallpapers

## Set icon in taskbar
