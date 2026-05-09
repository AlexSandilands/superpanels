# 9. Profiles, schedules & slideshows

## 9.1 Profiles

A profile is **the mode the user is in**. They have profiles like "home", "work-quiet", or "rgb-mode". The active profile is daemon state — exactly one at a time, persisted across restart. Switching profiles atomically swaps the image transform *and* the per-monitor canvas arrangement (`monitor_state`).

The on-disk shape is in §3.4. Highlights:

- `monitor_state: HashMap<StableId, MonitorPlacement>` — physical mm placement + per-monitor rotation for every monitor the profile expects.
- `topology: TopologyFingerprint` — opaque hash over the connected `stable_id`s + rotations the profile was authored against. Compared for equality at apply time; mismatch disables the profile until the user re-authors.
- `colour: ProfileColour` — one of 12 curated swatches.
- `created_at` / `updated_at` / `last_applied_at` — provenance for the manager UI.

When a profile is active, its `monitor_state` *is* the canvas: any drag, rotate, or image-transform tweak on the canvas auto-saves to the active profile. The "Save as new" top-nav button is the escape hatch — fork the current state into a new profile *before* further tweaks land on the active one.

### 9.1.1 Validity / disabled state

A profile is `Disabled` when *any* of:

- Topology mismatch (connected set or rotation differs from fingerprint).
- Referenced single image is missing.
- Referenced slideshow folder is missing or empty.
- Referenced `MonitorRef` in PerMonitor body is not connected.
- Required `physical_size_mm` missing for any expected monitor.

Disabled profiles are visible everywhere greyed-out, surface their disable reasons inline, and don't auto-apply when their schedule fires. Clicking opens the **topology-repair flow**: drop into the canvas with the *current* live monitor layout pre-populated, image reset to its `FitMode`-derived transform, and on save recapture `topology` + `monitor_state` from the live setup.

## 9.2 Slideshow

```rust
struct SlideshowConfig {
    interval: Duration,            // e.g. 30 minutes
    sort: SlideshowSort,           // Shuffle | Alphabetical | DateAsc | DateDesc | LastShownAsc
    recent_history_size: usize,    // suppress last N, default 10
    on_start: SlideshowStart,      // Resume | NewRandom | First
    pause_when_active: bool,       // pause the timer when the user switches images manually
    skip_on_unavailable: bool,     // if a file vanished between scan and apply, skip not error
}
```

Slideshow state (current index, history) is persisted in `$XDG_STATE_HOME/superpanels/slideshow-state.json` so it survives daemon restart and reboot.

## 9.3 Schedules

Schedules are a **top-level concept** that drive profile switches by clock — separate from the slideshow timer and lifted out of the profile body.

```rust
struct Schedule {
    display_name: Option<String>,
    profile: String,           // target profile (referenced by name)
    trigger: Trigger,
    enabled: bool,
}

enum Trigger {
    Daily   { hour: u8, minute: u8 },
    Sunset  { offset_minutes: i32 },     // requires Config.location
    Sunrise { offset_minutes: i32 },     // requires Config.location
    Cron    { expr: String },             // power-user escape hatch
}
```

The daemon maintains one Tokio timer per enabled rule, recomputing the next-fire time on config reload, system clock change, and after each fire.

### 9.3.1 Conflict prevention

Two enabled rules that fire at the same minute on a representative day are **unrepresentable in saved state**. The Settings → Schedules UI blocks save and the daemon's `save_schedules` IPC method rejects the request, naming the conflicting rule pair.

### 9.3.2 Boot catch-up

On daemon start, the schedule checker finds the most recent past fire-time today across all enabled rules and applies that rule's profile if the active profile differs. This prevents the silent failure mode where a 08:00 dark→light rule never fires because the user booted at 09:00.

### 9.3.3 Preemption

Schedule fires preempt manual choice. The escape is the master `schedules_paused` toggle (§9.5).

### 9.3.4 Sunset / sunrise approximation

Sun events are computed in-process using the Almanac approximation; accurate to ~±2 minutes at temperate latitudes, degrading near the polar day/night cutoff. Rules with `Sunset` / `Sunrise` triggers are disabled (with a `tracing::warn`) when `Config.location` is `None`.

## 9.4 Manual controls

- `superpanels next` / `superpanels prev` / `superpanels pause` / `superpanels resume` — slideshow.
- `superpanels schedule list / add / remove / enable / disable / pause / resume` — top-level schedule rules.
- `superpanels profile list / show / apply / create / edit / delete / rename / duplicate / export / import` — profile manager parity (`docs/spec/11-cli.md`).

All of the above are also IPC commands and are wired to the tray and GUI.

## 9.5 Schedules-paused master toggle

`Config.schedules_paused: bool` is a master switch. When `true`, the daemon's schedule checker is a no-op. Mirrored in:

- The tray menu's "Pause schedules" item.
- The Settings → Schedules tab's "Pause all schedules" checkbox.

The master toggle is the user's intentional escape from schedule preemption.
