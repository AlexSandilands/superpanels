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

## Black-line rendering artifact when dragging the window (intermittent)

Moving the GUI sometimes leaves black lines streaking from the bottom edge of
the window — they persist on the desktop and over other windows, not just inside
our app. **Intermittent**: not reproducible on demand (gone on a later session),
so it reads as a GPU/compositor buffer-presentation / damage-tracking glitch
rather than app layout.

Almost certainly the WebKitGTK-on-Wayland family of bugs we already mitigate with
`WEBKIT_DISABLE_DMABUF_RENDERER=1` (see the DMABUF follow-up above for the four
sites). Stale framebuffer regions leaking outside the surface is a known
WebKitGTK/Mesa/NVIDIA + KDE Plasma 6 failure mode.

**Candidate mitigations to try if it returns (cheapest first):**
- Confirm the existing `WEBKIT_DISABLE_DMABUF_RENDERER=1` is actually in effect
  for the way it's being launched (env vars only apply to `cargo run` / the
  justfile / the installed desktop entries — a bare `./superpanels-gui` won't
  have them).
- `WEBKIT_DISABLE_COMPOSITING_MODE=1` — heavier hammer; disables accelerated
  compositing for the webview (real smoothness/perf cost), but often clears
  stale-buffer artifacts. Gate behind testing before keeping.
- Track upstream WebKitGTK / `webkit2gtk` Arch package for buffer-damage fixes.

**Revisit when:** it reproduces reliably enough to A/B a mitigation.

## Remove `ProfileBody::PerMonitor`?

The user has never knowingly used the per-monitor mode and flagged it for
possible removal. It's the only body that doesn't share the unified canvas /
`monitor_state` model (it carries its own `assignments` + `fit`), so it's a
standing maintenance cost in every `ProfileBody` match across core/daemon/cli/gui.
**Revisit:** confirm nothing depends on it, then drop the variant + its apply
path, validity reasons, and frontend branches — or keep it if a real multi-output
"different image per screen" need surfaces.

## Add GUI Scale Option in Settings

Currently all the buttons and icons on the GUI are a set size, it would be nice to
allow the user to adjust the scale of the GUI, maybe between three options, compact, comfortable, and large.

## Draft-sync staleness in the preemption undo snapshot

A Standard draft's `body.layers` (and a Slideshow draft's `image_rect_mm`) is
only synced from the live canvas stores at apply/save (`syncDraftFromCanvas`),
so the schedule-preemption undo snapshot can capture a stale layer list / rect
if it fires mid-edit. **Revisit:** consider eager draft-sync (or
snapshot-on-apply) for the preemption undo. (An empty Standard is no longer a
failure case — it applies as an all-black desktop — so a stale-empty snapshot
just paints black rather than erroring.)

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

## Blocking config IO under the async mutex (daemon handlers)

`crates/superpanels-daemon/src/server/handlers.rs` (and siblings) call the
synchronous, blocking `Config::save_to` while holding the `tokio::sync::Mutex`
guard on `DaemonState` (e.g. `cmd_update_profile_image_transform`,
`cmd_update_profile_source`). It's a pervasive, pre-existing pattern — disk IO
stalls the async runtime and serialises every other handler behind the lock.
**Revisit:** move the write off the lock (clone the `Config`, drop the guard,
then `spawn_blocking` the `save_to`), or hold a dedicated blocking-IO path.
Needs a careful pass so the in-memory state and on-disk file can't diverge under
concurrent handlers.

## `PreviewCanvas.svelte` exceeds the component file-size cap

`crates/.../ui/src/components/canvas/PreviewCanvas.svelte` is ~460 lines against
the 350 soft / 350-ish component cap, carrying a `reason:` header that justifies
keeping projection + unified hit-testing together. **Revisit:** either ratify the
exemption explicitly (it is one coherent pointer/projection surface) or split the
pointer-routing state machine out from the projection/render setup. The Rust
`layout.rs` / `image.rs` over-cap is in-file `#[cfg(test)]` only (non-test bodies
are under the soft cap) and needs no action per the file-size-cap intent.

## Slideshow jump grid only shows library-indexed images

`slideshowJumpImages` (`ui/src/App.svelte`) builds the quick-jump grid from
`libraryStore.entries ∩ membershipLookup(set)` — i.e. only set images that are
*also* in the library index (the configured `library.roots`). A slideshow folder
that isn't a library root contributes nothing to the grid, even though the
daemon resolves it into the live pool (so the counter reads the full 224 while
the grid shows only the ~21 library-indexed ones).

**Fix:** give the jump grid the daemon's real resolved pool. The daemon already
has `resolve_pool` (`server/apply.rs`); add a thin `slideshow_pool` IPC command
that returns the resolved paths for a profile, wire an `api.slideshowPool`
binding, and have `slideshowJumpImages` use it (falling back to the library
intersection only when the daemon is unreachable). Avoid re-resolving folders on
the frontend — it would diverge from the daemon's recursive/sort/filter rules.
