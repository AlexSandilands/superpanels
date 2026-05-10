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

## OS-driven rotation push to GUI

The daemon subscribes to KDE's `org.kde.KScreen` `configChanged` D-Bus
signal (see `crates/superpanels-daemon/src/display_watch.rs`) and updates
its in-memory `monitors` and broadcasts `()` on `state.monitors_tx`. The
GUI listens for a Tauri event named `monitors://changed`
(`ui/src/lib/events/window.ts`) and calls `monitorStore.refresh()`.

What's missing: no plumbing relays the daemon's broadcast tick out over
the IPC socket as a Tauri event. Until that's wired, an OS-driven rotation
updates the daemon's view but not the running GUI — the user can hit F5
(`redetectMonitorsWithToast`) as a manual fallback. In-process mode (no
daemon) doesn't run `display_watch` at all.

**Action:** add a daemon→GUI push channel (e.g. a long-poll IPC method or
a separate socket) that emits Tauri events; or run a thin zbus subscriber
inside the GUI process for KDE sessions when no daemon is present.

## Cover `debounce_and_redetect` with a hermetic test

`crates/superpanels-daemon/src/display_watch.rs` `debounce_and_redetect`
collapses bursts of kscreen signals into a single re-detect (~250 ms
window). It currently has no unit test because zbus's `MessageStream` is
concrete and ties into a real `Connection`. Worth introducing a small
trait abstraction (e.g. `trait SignalSource: Stream<Item = ...>`) so a
test can drive coalescing under `tokio::time::pause()` and assert that
N signals within DEBOUNCE produce exactly one publish.

## Fix transforms/cropping from canvas
  - Should be pixel perfect according to canvas
  - Align with the preview in the profile manager
  - We should stop rendering the bezels in the canvas - it is breaking the math I think

## Slideshows

## Remove colors from profiles, not necessary anymore

## Review daemon logging - is it firing too frequently?
