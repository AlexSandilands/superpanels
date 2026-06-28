# Follow-ups

Loose ends that aren't blocking, but should be revisited. Add an entry when
deferring a workaround or "we'll do this later" item; remove it when done.

## WebKitGTK DMABUF renderer workaround

`WEBKIT_DISABLE_DMABUF_RENDERER=1` is set in four places to dodge a WebKitGTK
crash (`Gdk-Message: Error 71 (Protocol error)`) on Wayland under common
NVIDIA / Mesa + KDE Plasma 6 stacks:

- `.cargo/config.toml` — for `cargo run` / `cargo test`.
- `justfile` (`gui` recipe) — for direct release-binary invocation.
- `crates/superpanels-gui/src/autostart.rs` (`DESKTOP_BODY`) — for the
  installed `~/.config/autostart/superpanels.desktop` entry.
- `crates/superpanels-gui/src/desktop_entry.rs` (`desktop_body`) — for the
  app-menu `~/.local/share/applications/superpanels-gui.desktop` entry.

**Revisit when:** WebKitGTK ships a fix for the DMABUF renderer crash on the
affected stacks (track upstream WebKit / `webkit2gtk` Arch package).

**Action when fixed:** drop the env var from all four sites and the
`*_includes_webkit_dmabuf_workaround` / `set_enabled_at_true_writes_desktop_file`
test assertions in `autostart.rs` and `desktop_entry.rs`.

## Repair different topology

When the topology-repair flow lands, it must remap (or at minimum
invalidate, with a toast) the per-image `overrides` maps inside
`SlideshowSource` (`SlideshowProfile.source.overrides`), not just the
profile-level `monitor_state` — both are keyed by `stable_id`, so a monitor
swap silently breaks hand-tuned slideshow images otherwise.

## Misc Bugs

- When starting from fresh wiped local data, creating a new profile was disabled. Assume because no image and nothing library?
- Hovering over the snap buttons on the image in the canvas make them disappear
- Should add a resize handle to the top left of the image as well
- The library thumbnail on the bottom right library panel is empty when composite canvas
- Clicking a monitor now no longer shows the monitor details panel that used to pop up

- Should be able to filter the library by added folder
- After adding more images to an existing slideshow profile, the "x images" above the timer in the bottom right bar doesn't update to the right number of images. Switching profiles will update it
- Fix up icons
  - Library icon in top bar is the same as the move monitor toggle in side bar.
  - Should probably get a better settings icon
  - Have a look at the arrangement of the icons buttons and see what should be best placement
- The monitor gap text fields in the bottom bar are very hard to type in as it forces a format while typing
- In the profile manager, there is a "reveal" text beside the Source label, what is that for? Remove?
- System Tray Profiles don't show up, should have a hover side menu that opens the list
- Weird screen rendering artifacts when moving the Superpanels GUI around, it leaves black lines from the bottom of the app on the screen, both on the desktop and on top of other apps if it's in front
- The slideshow menu in the bottom right bar should have a popup to quickly let you switch to a particular image in the set. Currently you have to click through all of them to get to the one you want


## Remove `ProfileBody::PerMonitor`?

The user has never knowingly used the per-monitor mode and flagged it for
possible removal. It's the only body that doesn't share the unified canvas /
`monitor_state` model (it carries its own `assignments` + `fit`), so it's a
standing maintenance cost in every `ProfileBody` match across core/daemon/cli/gui.
**Revisit:** confirm nothing depends on it, then drop the variant + its apply
path, validity reasons, and frontend branches — or keep it if a real multi-output
"different image per screen" need surfaces.

## Draft-sync staleness in the preemption undo snapshot

A Standard draft's `body.layers` (and a Slideshow draft's `image_rect_mm`) is
only synced from the live canvas stores at apply/save (`syncDraftFromCanvas`),
so the schedule-preemption undo snapshot can capture a stale layer list / rect
if it fires mid-edit. **Revisit:** consider eager draft-sync (or
snapshot-on-apply) for the preemption undo. (The empty-canvas apply hole is
closed — `cmd_apply_canvas` now rejects an empty Standard, mirroring
`cmd_apply_profile`'s `standard_empty` gate and the GUI's `canApply`.)

## Daemon dies with its parent session

`daemonize()` (`crates/superpanels-daemon/src/main.rs`) re-execs with
`--foreground` but never calls `setsid()`, so the "background" daemon stays
in the launching session/process group: started from a terminal it dies with
that terminal's SIGHUP, and stderr is nulled so it dies silently. Packaged
installs should prefer the systemd user unit (`--install-unit`); for the
bare-binary path, detach properly (double-fork + `setsid` via a small
`nix`-free mechanism, or re-exec under `setsid(1)` when available).

## Startup re-apply has no retry

The daemon applies its initial profile (resume or `default_profile`) once,
500 ms after boot, to allow compositor readiness. At session login that race
is real: if plasmashell isn't up yet the apply fails with a single `warn!`
and the wallpaper is whatever the compositor cached. Consider a short
retry-with-backoff (e.g. 3 attempts over ~5 s) before giving up.

## Title-bar status dot is decorative

`TitleBar.svelte` renders `<span class="dot ok">` unconditionally — it reads
as a health indicator but means nothing. Tie it to `daemonStatus.connected`
(and consider an amber state while `starting`) or drop it.
