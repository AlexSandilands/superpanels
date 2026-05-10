# Phase 4e — Profiles redesign (v0.8)

**Goal.** Profiles become a coherent "mode the user is in": one image transform, one physical monitor arrangement, one colour swatch. Schedules lift out of profiles into a top-level concept with their own settings UI. A dedicated profile manager window replaces the cramped tray selector for CRUD. The tray selector itself becomes a clean switcher.

Slots in between Phase 4d and Phase 5 (packaging shifts to v0.9). Design doc: `docs/design/profiles-redesign.md` — fold its sections into `docs/spec/03`, `docs/spec/09`, `docs/spec/12` as part of 4e.9 and delete the working doc.

**Visual source of truth:** `docs/design/mockup/superpanels-claude/`. Open `index.html` to view; per-component file:line references are below in 4e.5 / 4e.6 / 4e.7 / 4e.7b. Only profile-related surfaces in the mockup are current — see the mockup's `README.md` for the authoritative/ignored split.

**No migrations.** Pre-1.0; local state is wiped on schema change. Don't add config-version shims.

**Definition of done.**
- [x] Switching profiles swaps both the image transform *and* the per-monitor canvas arrangement (`xMm`, `yMm`, `rotation`) atomically.
- [x] `BezelConfig` is gone from the codebase. Gaps are derived from authored placements.
- [x] Profiles store `created_at`, `updated_at`, `last_applied_at`, a colour swatch, and a topology fingerprint.
- [x] Disabled profiles (topology mismatch, missing image, missing folder, etc.) appear greyed-out in selector and manager; clicking opens the repair flow.
- [x] Schedules live at config top-level. Daemon honours them, preempts manual choice, applies most-recent past rule on boot, and supports `schedules_paused`.
- [x] Settings → Schedules is fully populated; UI blocks save when two rules collide at the same minute.
- [x] Profile manager opens as a library-shaped overlay (same pattern as `LibraryModal`) with full CRUD, duplicate, import/export, search, thumbnails, validity badges. (Spec originally called for a dedicated Tauri window; revisited in implementation — overlay avoids extra capability surface and IPC duplication.)
- [x] Tray selector closes on outside click, truncates long names with tooltip, sorts by recency, and shows a sensible empty state. Selector is switch-only; CRUD lives in the manager and the top-nav save button.
- [x] Top nav has a "Save as new profile" button beside Apply that captures the live canvas state into a new profile.
- [x] CLI has parity with manager actions; `superpanels schedule …` subcommand exists.
- [x] Working design doc deleted; relevant content folded into `docs/spec/03`, `docs/spec/09`, `docs/spec/12`.
- [x] Save model revised: Apply ephemeral; Save / Save-as-new / Revert button cluster; confirm-discard modal on user switch + window close; schedule preemption surfaces a toast with Undo. (See §4e.11.)

---

## 4e.1 Core schema redesign

- [x] Drop `BezelConfig` from `superpanels-core` entirely. Remove the type, its serde, all references in core, daemon, GUI, CLI, and tests.
- [x] `Profile` gains: `colour: ProfileColour`, `description: Option<String>`, `topology: TopologyFingerprint`, `monitor_state: HashMap<StableId, MonitorPlacement>`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`, `last_applied_at: Option<DateTime<Utc>>`.
- [x] `Profile` loses: `bezels`, `schedule`.
- [x] New `MonitorPlacement { x_mm: f32, y_mm: f32, rotation: Rotation }`. Mirrors the existing transient `MonitorOverride` from `ui/src/lib/stores/canvas-view.svelte.ts`.
- [x] New `TopologyFingerprint(String)` — opaque hash over (sorted `stable_id`s ∥ rotations). Hash function and stability rules documented; deterministic across processes.
- [x] New `ProfileColour` — enum over a curated palette (~12 swatches).
- [x] `Config` gains: `schedules: Vec<Schedule>`, `schedules_paused: bool`. (Sun-event triggers + lat/long dropped in §4e.12.)
- [x] `Schedule` enum lifted out of profile body, gains `enabled: bool`, optional `display_name: String`, target `profile: String`.
- [x] Bezel-math layout step (`compute_crop_specs` and friends) consumes profile `monitor_state` instead of detected positions + `BezelConfig`. Live OS arrangement is consulted only for: connected set, pixel resolutions, OS rotation (for fingerprint comparison).
- [x] Unit tests: round-trip TOML for the new shape; topology hash determinism; placement round-trip with rotation.
- [x] Property tests (proptest): random monitor sets + placements still produce valid bezel-math output.

## 4e.2 Validity & topology

- [x] `ProfileValidity { Ok | Disabled { reasons: Vec<DisableReason> } }` derived (not stored). Reasons: `TopologyMismatch`, `ImageMissing(PathBuf)`, `FolderMissingOrEmpty(PathBuf)`, `MonitorNotConnected(MonitorRef)`, `PhysicalSizeMissing(StableId)`.
- [x] Validity is recomputed on apply, on monitor-set change, on config reload. Daemon publishes per-profile validity in IPC responses (`list_profiles`, `current_state`).
- [x] Apply path refuses disabled profiles with a structured error; the GUI surfaces the reasons inline.
- [x] **Topology-repair flow.** When the user clicks a disabled profile in the selector or manager:
  - [x] Open the canvas with the *current* live monitor layout pre-populated.
  - [x] Reset image to its original `FitMode`-derived transform (drop `image_size_px`, drop `offset`).
  - [x] On Save: recapture `topology` and `monitor_state` from the live setup; the profile becomes Ok.
- [x] Tests: each disable reason triggers correctly; repair flow produces a re-Ok'd profile.

## 4e.3 Canvas state — per-profile auto-save

> **Superseded by §4e.11 (2026-05-10).** Auto-save was removed; Apply is now ephemeral. Tickboxes below describe the original implementation; see §4e.11 for the current model.

The model: when a profile is active, the canvas *is* the profile. Every canvas change auto-saves. The top-nav "Save as new" button (4e.7b) is the only way to fork without overwriting.

What auto-saves to the active profile:

- [x] Monitor placement (drag-release): `xMm`, `yMm`.
- [x] Monitor rotation.
- [x] Image transform: `offset`, `image_size_px`.
- [x] Image source (drag-and-drop or library pick replaces the existing image on the active profile).
- [x] Reset-overrides: re-pulls live OS layout *and* writes back. Reset is a save.

Wiring:

- [x] `MonitorOverride` in `canvas-view.svelte.ts` becomes a *projection* of the active profile's `monitor_state`. The local `overrides` record is derived, not authoritative.
- [x] Add IPC: `update_profile_monitor_state(profile_name, stable_id, placement)`, `update_profile_image_transform(profile_name, transform)`, `update_profile_source(profile_name, source)`. All bump `updated_at`.
- [x] Debounce sub-second drags so we don't write a TOML file per pixel; flush on drag-release.
- [x] No-profile state (empty config / first-run): drags are local-only and dead-end into the "Create your first profile" CTA — same UX as today's transient overrides.
- [x] Test: programmatic drag in the GUI test harness persists to the underlying profile TOML; image swap likewise persists; debouncing observed.

## 4e.4 Schedules — daemon runtime

- [x] `superpanels-daemon` reads `Config.schedules` on start and on config reload (already watching).
- [x] One Tokio timer per enabled rule. Recompute next-fire on reload, on system clock change, and after a fire.
- [~] Sunset/sunrise: dropped in §4e.12 — daily + cron only.
- [x] Cron rules: pull in `cron` or `croner` crate (decide in 4e.4 review). Validate cron expressions on save; reject invalid ones at the IPC boundary.
- [x] **Boot catch-up:** on daemon start, find the most recent past fire-time today across all enabled rules; apply that rule's profile if the active profile differs.
- [x] Schedule preempts manual choice unless `schedules_paused` is true.
- [x] If a schedule targets a disabled profile, log + skip; do not switch.
- [x] Persist `active_profile` and `schedules_paused` in `state.toml`.
- [x] Tests: integration test for boot catch-up; pause toggle observed.

## 4e.5 Schedules — settings UI

Mockup reference: `docs/design/mockup/superpanels-claude/overlays.jsx` — `SchedulesSettings` (`:430`), `ScheduleRowFull` (`:496`), `ScheduleEditor` (`:524`).

- [x] Settings → Schedules tab populated. Currently empty.
- [x] Rule list. Each row: enabled toggle, trigger summary, target profile, "next fires at HH:MM" hint, edit/delete.
- [x] Add-rule form: trigger type (daily / cron), parameters, target profile dropdown (existing profiles only). (Sun-event triggers dropped in §4e.12.)
- [x] **Conflict prevention:** save is blocked when a new/edited rule would fire at the same minute as another enabled rule. Inline error names the conflicting rule. Two rules at the same minute is unrepresentable in saved state.
- [x] Master "pause all schedules" toggle (mirrored in tray).
- [x] Tests: vitest covers conflict-detection logic; e2e (or component-level) covers add → list → next-fire formatting.

## 4e.6 Profile manager window

Mockup reference: `docs/design/mockup/superpanels-claude/profiles.jsx` — `ProfileManager` (`:171`), `ProfileRow` (`:376`), `ColorPopover` (`:427`), `EmptyState` (`:448`), `MonitorMini` topology preview (`:38`), `ConfirmDialog` reusable (`:148`), curated `SWATCHES` palette (`:6`).

- [x] Modal overlay (`ProfileManagerModal`), opened from the tray ("Open profile manager…") and from a top-nav button. Originally specced as a dedicated Tauri window; collapsed to an overlay in implementation.
- [x] Shape mirrors `LibraryModal` (left rail / main pane / detail pane on a `Backdrop`).
- [x] List view per profile: thumbnail (reuse `cmd_library_thumbnail`), name, colour swatch, last-applied recency, validity badge, "Authored for: {topology}" chip when current topology differs.
- [x] Search / filter input; case-insensitive name match.
- [x] Per-profile actions: Apply, Rename (inline), Edit colour swatch (palette popover), Edit description, Open referenced file/folder in OS file manager (`tauri-plugin-opener` already in deps), Duplicate, Export (TOML bundle), Delete (confirm dialog).
- [x] Top-level actions: New profile, Import bundle, Empty-state CTA.
- [x] Disabled-profile rows: greyed, validity reasons listed, "Repair" button → topology-repair flow (4e.2).
- [x] Tests: vitest for the list filtering / sorting; component test for delete confirm; manual checklist for the file-manager open action.

## 4e.7 Tray selector fixes

Mockup reference: top-nav profile pill in `chrome.jsx:24-45`, dropdown body `TraySelector` (`:124`). System-tray right-click menu: `overlays.jsx:650` (`TrayPopover`).

The tray selector is purely for **switching**. Creation actions live elsewhere (manager for blank/duplicate/import; top-nav button for capture-current).

- [x] **Outside click closes the dropdown.** Currently clicking the canvas does not dismiss it. Fix the focus/blur or document-click wiring.
- [x] Long names truncate with ellipsis + native tooltip.
- [x] Sort by `last_applied_at` desc; pinned/active profile first.
- [x] Empty state: replace the broken "no profiles" view with a clear CTA: "No profiles yet — open the profile manager."
- [x] Surface the active schedule rule when present: "Auto: switching to dark at 18:00".
- [x] Add menu items: "Open profile manager…" and "Pause schedules" toggle.
- [x] Remove the in-selector "New profile" / "Save current as new" shortcuts. Manager owns blank/duplicate/import; the new top-nav save button (4e.7b) owns capture-current.

## 4e.7b Top-nav "Save as new profile" button

> **Re-framed by §4e.11 (2026-05-10).** Save-as-new is no longer the only escape from autosave; it now sits alongside Save, Revert, and the confirm-discard modal.

Mockup reference: save-icon button in `chrome.jsx:67-71` (TitleBar), dialog component `SaveProfileDialog` (`profiles.jsx:66`).

The auto-save model in 4e.3 means the active profile silently mutates as the user drags monitors or repositions the image. This button is the escape hatch — fork the current state into a new profile *before* further tweaks land on the active one.

- [x] Add a save-icon button in `TitleBar.svelte`, immediately beside the existing Apply button.
- [x] **Click opens a small dialog** with two required fields and one optional:
  - Name (required, validated for uniqueness against existing profiles)
  - Colour swatch (required, curated palette picker)
  - Description (optional)
  Defaults: name = `"<active>-copy"` or `"untitled-N"` if no active profile; colour = next unused palette swatch.
- [x] Confirm → creates a new profile capturing the *current* canvas state (image source, transform `offset` + `image_size_px`, `monitor_state`, `topology` from live OS) and switches to it as active.
- [x] Cancel → no-op; canvas unchanged.
- [x] Disabled (with tooltip "no image on canvas") when there's nothing to capture.
- [x] Goes through the same core `create_profile` operation as the manager and CLI.
- [x] Tests: dialog name-uniqueness validation; new profile carries the active monitor placements and image transform verbatim; the previously-active profile is unchanged.

## 4e.8 CLI parity + ts-rs

- [x] `superpanels profile` subcommand: ensure `list, show, apply, create, edit, delete, rename, duplicate, export, import` all exist and go through the same core operations as the GUI. Fill gaps.
- [x] New `superpanels schedule` subcommand: `list, add, remove, enable, disable, pause, resume`.
- [x] All new core types (`MonitorPlacement`, `TopologyFingerprint`, `ProfileColour`, `Schedule`, `ProfileValidity`, `DisableReason`) exported via `ts-rs`. Drop the hand-mirrored types in `ui/src/lib/types/profile.ts` once `ts-rs` covers everything; until then update by hand to match.
- [x] Tests: CLI integration tests for each new verb hitting an in-process core (no daemon).

## 4e.9 Spec fold & doc cleanup

- [x] `docs/spec/03-core-concepts.md` §3.4 Profile: rewrite to match new shape. Drop bezels, add `monitor_state`, colour, timestamps, topology.
- [x] `docs/spec/09-profiles-schedules.md`: schedules promoted to top-level. §9.1 Profiles trimmed; §9.3 Schedules describes the new shape including conflict-prevention rule and boot catch-up. Add a §9.5 for `schedules_paused`.
- [x] `docs/spec/12-gui.md`: document the profile manager window, tray selector behaviour, schedules settings UI.
- [x] `docs/spec/04-bezel-math.md`: clarify gaps are derived from authored `monitor_state`, not from a separate bezel field.
- [x] Update `docs/architecture.md` if module boundaries shift (likely a new `superpanels-core/src/schedule.rs` and `superpanels-gui/src/profile_manager/`).
- [x] Delete `docs/design/profiles-redesign.md`.

## 4e.10 Carry-overs to track

- [x] If `Schedule::Cron` validation needs a non-trivial dep, log a `cargo-deny` follow-up.
- [x] Topology-fingerprint stability under unrelated KDE updates — note in `docs/followups.md` to revisit if false-disable reports surface.

## 4e.11 Save model revision (post-launch amendment)

Phase 4e shipped with the canvas auto-saving every drag, drop, and rotation onto the active profile. After live use this turned out to be the wrong model: it makes Apply redundant, prevents experimentation, and forces users to type a new name (`Save as new`) before *any* tweak that they don't want to keep. The corrected model below is dated 2026-05-10 and supersedes §4e.3.

The new contract:

- **Apply** = ephemeral push of the current canvas state to the desktop. Does NOT mutate the active profile's TOML.
- **Save** = commit the current canvas state to the active profile. New button.
- **Save As New** = unchanged (§4e.7b). Forks current canvas into a new profile.
- **Revert** = re-pulls active profile state into the canvas, discarding local edits. New button.
- Auto-save is dropped.

Visibility / enable rules:

- Apply, Save, Save As New, Revert — all always visible.
- Save — disabled when there is no active profile; rendered with the default white tint when the canvas is clean and tinted with `--accent` when it is dirty.
- Revert — disabled when the canvas is clean OR when there is no active profile.
- Save As New — disabled rules unchanged from §4e.7b ("no image on canvas" tooltip).
- Apply — disabled rules unchanged.

### 4e.11.1 Apply becomes ephemeral

- [x] Add a daemon IPC method `apply_canvas` that takes a transient canvas payload (image source, transform, `monitor_state`, optional `backend_override`) and runs the same `run_span_apply` / `run_per_monitor_apply` pipelines without writing to `config.profiles`. Mirrored in the GUI's `commands/profiles.rs` Tauri shell.
- [x] Drop autosave from the Apply path. `apply_profile(name)` keeps its existing semantics (no canvas snapshot, applies whatever's persisted) and remains the entry point used by schedule fires, the manager-list Apply action, and the boot catch-up.
- [x] Frontend `applyDraftProfile()` calls `apply_canvas` instead of `save_profile + apply_profile`. The active profile name (if any) is passed alongside so the daemon can update `last_applied_at` without rewriting `monitor_state`.
- [x] Tests: integration test that `apply_canvas` updates the desktop but does not change `config.profiles[active].monitor_state`; another that `apply_profile` still applies the persisted state verbatim.

### 4e.11.2 Drop per-profile autosave wiring

- [x] Remove the eager `update_profile_monitor_state` / `update_profile_image_transform` / `update_profile_source` calls from canvas drag/drop/transform handlers in the frontend. The autosave wiring lived in `ui/src/lib/canvas/transform-actions.ts` plus the in-store `setSpanImage` / `pinImageToMonitor` flows. (None were actually invoked at the time of the revision — the autosave wiring listed in §4e.3 was specced but never connected — but we kept the IPC stubs in place for the Save action / topology-repair / settings panes per the original brief.)
- [x] The IPC commands themselves stay defined (used by Save, by the topology-repair flow, and the Settings panes). Keep the Rust-side handlers; only the unconditional client invocations go away.

### 4e.11.3 Save (existing) button

- [x] Add a `save` button to the `TitleBar` cluster that calls `save_profile` for the currently-active profile. Disabled when there is no active profile.
- [x] Dirty detection: a `$derived` flag in `ui/src/lib/stores/profile.svelte.ts` that compares the live canvas state (draft body + image transform + canvas-view overrides) to the active profile's persisted state. The flag drives the accent-colour swap on the Save icon. (Image transform diff deferred — see `docs/followups.md`.)
- [x] When the Save fires, the active profile's `monitor_state`, image transform, and `topology` are snapshotted from the live canvas — same payload shape that Save-as-new uses.

### 4e.11.4 Revert button

- [x] Add a `revert` button to the cluster that re-pulls the active profile's persisted state into the canvas (overrides + draft + image-transform store). Equivalent to `profileStore.revertToActive()` plus `applyMonitorStateToCanvas(active)`.
- [x] Disabled when the canvas is clean OR when there is no active profile.

### 4e.11.5 Confirm-discard modal

- [x] Reuse the existing `ConfirmDialog` widget (`ui/src/components/widgets/ConfirmDialog.svelte`) wrapped behind a thin helper. New file: `ui/src/components/overlays/ConfirmDiscardModal.svelte`.
- [x] Triggers on **user-initiated** profile switch (tray pill, profile-manager Apply, top-nav profile selector) when the canvas is dirty.
- [x] Triggers on the main window close-request (Tauri `WindowEvent::CloseRequested`) when the canvas is dirty. The frontend's `onCloseRequested` handler vetoes the close (calls `event.preventDefault()`), pops the modal, and re-issues `winRef.destroy()` after the user confirms.
- [x] Confirm = drop edits and proceed; Cancel = stay on the current canvas.

### 4e.11.6 Schedule preemption toast

- [x] Schedule preemption (§9.3.3) is **unchanged** — schedules still preempt manual choice when not paused.
- [x] When a schedule fires while the canvas is dirty, do not pop the modal. Instead, the frontend tracks a `userActiveSentinel`; when polling sees `active_profile` differ from the sentinel and a dirty canvas snapshot is buffered, surface a toast: "Schedule switched to {profile}; unsaved changes to {prev} were discarded" with an "Undo" action button.
- [x] The Undo action re-applies the snapshotted previous canvas state via `apply_canvas` and restores it as the local canvas. Ephemeral — it does not save.
- [x] Tests: vitest covers `canvasOverridesDirty` (the dirty-diff helper); a focused vitest for the schedule preemption sentinel→toast wiring is deferred to follow-up because the sentinel logic lives inside `App.svelte` `$effect`s and the most useful test target is the helper. Track a follow-up to extract it for direct testing.

## 4e.12 Sun-event removal (post-launch amendment)

Sunset / sunrise triggers were specced and shipped in §4e.4 / §4e.5, but they brought non-trivial weight (LatLong type, hand-rolled almanac approximation, location config field, lat/long input UI requirement) for marginal value. Removed entirely on 2026-05-10 — `Trigger` is now `Daily | Cron` only.

Removed surface:

- [x] `Trigger::Sunset`, `Trigger::Sunrise` variants (core).
- [x] `LatLong` type and ts-rs binding.
- [x] `Config.location` field.
- [x] `sun_event_utc_minutes` and `ScheduleError::LocationMissing`.
- [x] Daemon `sun_should_fire` / `sun_fire_local` helpers.
- [x] UI sun-trigger segment in `ScheduleEditor.svelte` and the corresponding row formatter in `ScheduleRow.svelte`.
- [x] CLI sun-trigger description branch.
- [x] Spec §9.3.4 and the lat/long line in §12.4.4.
