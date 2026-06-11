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
`SpanSource::Slideshow`, not just the profile-level `monitor_state` — both
are keyed by `stable_id`, so a monitor swap silently breaks hand-tuned
slideshow images otherwise.

## Per Monitor Wallpapers

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
