# 9. Profiles, schedules & slideshows

## 9.1 Profiles

A profile is the unit the user thinks in. They have profiles like "home" or "work-quiet" or "rgb-mode". Switching profiles is one click in the tray.

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

Slideshow state (current index, history) is persisted in `$XDG_STATE_HOME/superpanels/state.json` so it survives daemon restart and reboot.

## 9.3 Schedules

Time-of-day triggers, separate from the slideshow timer:

```rust
enum Schedule {
    Daily { at: TimeOfDay, profile: String },              // e.g. "switch to dark profile at 18:00"
    Sunset { offset: Duration, profile: String },          // requires lat/long; sunset/sunrise via approximation
    Cron(String),                                          // power-user escape hatch
}
```

A profile can have a schedule, or a global schedule list can flip between profiles. Both forms are valid.

## 9.4 Manual controls

- `superpanels next` — advance the slideshow one step (works even if the daemon isn't running; falls back to in-process).
- `superpanels prev` — go back.
- `superpanels pause` / `superpanels resume`.
- All of the above are also IPC commands and are wired to the tray and GUI.
