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

## Phase 4e carry-overs

- **Topology fingerprint stability under unrelated KDE updates.** The
  fingerprint hashes `stable_id` ∥ rotation. KDE's per-output UUID is
  expected to be stable across normal desktop sessions, but a major KDE
  refactor (or a switch to a different display manager) could change the
  UUID format. Watch for false-disable reports against valid setups; if
  they surface, consider hashing a normalised `(manufacturer, model,
  serial)` triple as a fallback.
- **`schedules_paused` persistence location.** Currently lives in
  `config.toml` for simplicity. The plan called for `state.toml`. Move
  if/when we introduce a proper `state.toml` (today only the slideshow
  state lives in `$XDG_STATE_HOME`).
- **`cron` crate vs `croner`.** We picked `cron` for the schedule
  validation path. If the expression dialect needs to grow (timezones,
  nicknames like `@daily`), reassess; `croner` may be nicer.
- **ts-rs auto-emit on build.** Generated bindings live next to the
  hand-written types in `ui/src/lib/types/`. Generation happens via
  `cargo test export_bindings`; fold this into the dev workflow / CI.
- **Repair flow UX.** Hooks exist (validity surfaces reasons, manager
  shows a Repair button) but the click-through flow that pre-populates
  the canvas with the live monitor layout is implemented at the IPC
  level, not yet polished in the canvas UI. Track UX feedback.
- **Schedule preemption sentinel test (4e.11.6).** The
  `userActiveSentinel` + dirty-snapshot logic that surfaces the
  schedule-preemption toast lives inside `App.svelte` `$effect`s; the
  vitest harness can't drive Svelte runes directly. Extract the
  decision into a pure module (input: previous sentinel, runtime
  active, dirty snapshot; output: toast payload | none) and write the
  test against it. Until then, the dirty-diff helper has direct
  coverage in `dirty.test.ts` but the sentinel→toast hand-off is only
  exercised manually.
- **Image transform not in dirty diff (4e.11.3).** `canvasOverridesDirty`
  in `ui/src/lib/canvas/dirty.ts` only diffs the per-monitor placements;
  the image transform stays in `imageTransform.value` (mm-space) while
  the persisted profile stores `offset` and `image_size_px` in pixels.
  Closing the gap needs a mm↔px converter (probably by sharing the
  layout-bbox math in `preview-layout.ts`). When wired, the Save button's
  accent-tint and the Revert button's enable rule should switch on the
  combined diff. Until then, image-only edits don't tint the Save icon
  and Revert is disabled despite there being something to revert.
