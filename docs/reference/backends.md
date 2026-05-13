# Wallpaper backends

How Superpanels applies the rendered crops to the compositor. Code lives in [`crates/superpanels-core/src/backends/`](../../crates/superpanels-core/src/backends/). This doc is the trait contract + per-backend specifics + subprocess safety rules every backend must follow.

## Trait

```rust
pub struct AppliedReport {
    pub monitors_set: usize,        // count of monitors covered (composite-aware)
    pub duration: Duration,         // wall-clock of the apply
    pub backend: &'static str,      // diagnostics
}

pub enum BackendError {
    Unavailable { backend: &'static str, reason: String },
    Subprocess { cmd: String, exit: i32, stderr: String },
    Timeout { cmd: String, seconds: u64 },
    DBus(String),
    UnknownMonitor(String),
    Encode(String),
}

/// `Send + Sync` so the daemon can hold one in an `Arc` and dispatch from
/// multiple threads without a `Mutex`.
pub trait WallpaperBackend: Send + Sync {
    fn name(&self) -> &str;
    /// Cheap check — env vars and PATH only, no subprocess spawn.
    fn availability(&self) -> Availability;
    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError>;
    /// `false` for backends that composite then set one image.
    fn supports_per_monitor(&self) -> bool;
}
```

`apply()` returns `AppliedReport` rather than `()` so callers (toast surface, daemon audit log) can surface count and duration without a breaking change. Backends that don't support per-monitor (older GNOME) composite the per-monitor crops into one bezel-correct image; `monitors_set` still reflects the number of monitors covered.

## Auto-detect order

| Priority | Backend | Condition |
|---|---|---|
| 1 | KDE | `$KDE_FULL_SESSION == "true"` or `$XDG_CURRENT_DESKTOP` contains `KDE` |
| 2 | Hyprland | `$HYPRLAND_INSTANCE_SIGNATURE` set |
| 3 | Sway / wlroots | `$SWAYSOCK` set, or `swww` / `swaybg` in `$PATH` |
| 4 | GNOME | `$XDG_CURRENT_DESKTOP` contains `GNOME` |
| 5 | feh | `$DISPLAY` set and `feh` in `$PATH` |
| 6 | Custom | `backend.custom_command` set in config |

A non-`Auto` `backend.prefer` in config is honoured unconditionally — the apply-time error is more actionable than a silent fallback. When `Auto` finds nothing, `detect_backend` returns an `UnavailableBackend` sentinel that errors loudly on `apply`.

## Subprocess rules (every backend follows these)

- `std::process::Command` only — never `shell = true`, never string concatenation.
- Always set a **10-second timeout**.
- Always check `.status().success()`; return `BackendError::Subprocess` with stderr included on failure.
- File paths are passed as `OsStr` arguments via `Command::arg(...)`, never interpolated.
- All commands run with the inherited environment plus an explicit `LC_ALL=C` so we can parse output reliably.

Shared subprocess helper: `crates/superpanels-core/src/backends/subprocess.rs`.

## Per-backend specifics

### KDE (`backends/kde.rs`)

`zbus`-backed D-Bus call to `org.kde.PlasmaShell.evaluateScript` setting the per-monitor `Image` plugin source. The JS payload is a versioned template; we generate it server-side, never accept it from user input. **Image paths are JSON-quoted into the script template**, never string-concatenated.

KDE Plasma 6 has no `outputName` on `Desktop`; we iterate assignments by connector and resolve via `screenForConnector`.

### Hyprland (`backends/hyprland.rs`)

`hyprctl hyprpaper preload PATH` followed by `hyprctl hyprpaper wallpaper "MONITOR,PATH"` per monitor. Requires `hyprpaper` already running — we don't start it.

### Sway / wlroots (`backends/sway.rs`)

Prefer `swww` (smooth fades), fall back to `swaybg`. `swww img --outputs DP-1 path.png` for per-monitor.

### GNOME (`backends/gnome.rs`)

Composite the per-monitor crops into a single bezel-correct image of the spanning canvas, then `gsettings set org.gnome.desktop.background picture-uri[-dark] file://…`. The composite is sized to the *logical* desktop, not the physical one.

### feh (`backends/feh.rs`)

`feh --bg-fill IMAGE1 IMAGE2 …` — feh handles per-monitor compositing.

### Custom (`backends/custom.rs`)

Shell-command template from config with `{image_N}` / `{monitor_N}` placeholders. Runs with the same subprocess rules; the user is responsible for command safety. The GUI's custom-command field shows a "this runs with your privileges" callout.

## Feature flags

Some backends pull weight (zbus is ~1 MB compiled). They are gated behind Cargo features (`backend-kde`, `backend-gnome`, …), all on by default. Minimal-distro packagers can disable some.

## Mock backend

`MockBackend` (in `backends/mock.rs`, gated on `#[cfg(any(test, feature = "test-support"))]`) records `(MonitorRef, PathBuf)` pairs in a `Mutex<Vec<_>>` and returns a successful `AppliedReport`. **All integration tests for the apply pipeline use it; we never touch a real desktop in tests.**
