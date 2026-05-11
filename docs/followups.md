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

## Review OS-rotation push for perf

The daemon now runs a 2 s `kscreen-doctor` poll in `display_watch.rs` plus
a long-poll IPC subscriber thread in the GUI
(`commands/monitors.rs::spawn_push_relay`). Worth measuring once the
feature is in real use: subprocess cost of the poll on idle, daemon-CPU
under sustained rotation churn, and whether the GUI's dedicated thread
holds a connection cleanly across daemon restarts.

## Pin a real D-Bus signal for rotation (drop the 2 s poll latency)

`display_watch.rs` ships a 2-second polling backstop because Plasma 6
Wayland on this stack doesn't appear to emit the legacy `org.kde.KScreen`
`configChanged` signal (the kscreen kded module is often unloaded, and
its DBus surface doesn't expose the interface even when loaded). The
D-Bus subscriber is still wired but unused in practice on Plasma 6.

**Action:** run `dbus-monitor --session "type='signal'"` while rotating a
display in System Settings, identify what actually fires (likely a KWin
or kded signal we haven't pinned), and update `build_match_rule` to
target it. Once verified, the poll cadence can be relaxed to e.g. 10 s
as a safety net only.

## Fix transforms/cropping from canvas
  - Should be pixel perfect according to canvas
  - Align with the preview in the profile manager
  - We should stop rendering the bezels in the canvas - it is breaking the math I think

## Slideshows

## Monitor gap not loaded on app start for profile

## Repair different topology
