# 10. Backend system

## 10.1 Trait

```rust
pub trait WallpaperBackend: Send + Sync {
    fn name(&self) -> &str;
    fn availability(&self) -> Availability;
    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError>;
    fn supports_per_monitor(&self) -> bool;
}

struct AppliedReport {
    /// Number of monitors successfully assigned a wallpaper. For composite
    /// backends (GNOME) this is the count of monitors covered by the composite.
    monitors_set: usize,
    /// Wall-clock duration of the apply, including any subprocess + redraw wait.
    duration: Duration,
    /// Backend name that handled the apply (for diagnostics).
    backend: &'static str,
}

#[derive(Debug, thiserror::Error)]
enum BackendError {
    #[error("backend `{backend}` is not available: {reason}")]
    Unavailable { backend: &'static str, reason: String },
    #[error("subprocess `{cmd}` failed (exit {exit}): {stderr}")]
    Subprocess { cmd: String, exit: i32, stderr: String },
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout { cmd: String, seconds: u64 },
    #[error("D-Bus call failed: {0}")]
    DBus(String),
    #[error("monitor `{0}` not present in current layout")]
    UnknownMonitor(String),
}
```

`availability()` must be cheap: env var check or `which` lookup. Never spawn a process. The enum (defined in §6.1) lets `superpanels detect --debug` explain *why* a backend was skipped — the same diagnostic value as the detector trait.

`apply()` receives `(monitor, image)` pairs and returns an `AppliedReport` rather than `()` so future callers (the GUI toast, the daemon's audit log) can surface the count and duration without a breaking trait change. Backends that don't support per-monitor (older GNOME) composite the per-monitor crops into one large image and set it as the spanning wallpaper; `monitors_set` still reflects the number of monitors covered.

## 10.2 Auto-detection order

| Priority | Backend | Detection condition |
|---|---|---|
| 1 | KDE | `$KDE_FULL_SESSION == "true"` or `$XDG_CURRENT_DESKTOP` contains `KDE` |
| 2 | Hyprland | `$HYPRLAND_INSTANCE_SIGNATURE` set |
| 3 | Sway / wlroots | `$SWAYSOCK` set or `swww` / `swaybg` in `$PATH` |
| 4 | GNOME | `$XDG_CURRENT_DESKTOP` contains `GNOME` |
| 5 | feh | `$DISPLAY` set and `feh` in `$PATH` |
| 6 | Custom | `backend.custom_command` set in config |

User can pin a backend in config (`backend.prefer = "kde"`) to skip detection.

## 10.3 Subprocess rules (every backend follows these)

- `std::process::Command` only — never `shell = true` string concatenation.
- Always set a 10-second timeout.
- Always check `.status().success()`; return `Err` with stderr included.
- File paths are passed as `OsStr` arguments, never interpolated.
- All commands run with the inherited environment plus an explicit `LC_ALL=C` so we can parse output reliably.

## 10.4 Per-backend specifics

- **KDE.** `zbus`-backed D-Bus call to `org.kde.PlasmaShell.evaluateScript` setting per-monitor `Image` plugin source. The JS payload is a versioned template string with placeholder substitution; we generate it server-side, never accept it from user input.
- **GNOME.** `gsettings set org.gnome.desktop.background picture-uri[-dark] file://…`. Multi-monitor strategy is *to be verified in Phase 2* — modern GNOME Shell may support per-monitor wallpapers via Mutter's backend, in which case the per-monitor pipeline applies directly. The fallback (and current assumption) is to composite the per-monitor crops into a single bezel-correct image of the spanning canvas; GNOME then displays that image stretched across the desktop region. The composite is sized to the *logical* desktop, not the physical one. See PLAN Phase 2 risks.
- **Hyprland.** Uses `hyprctl hyprpaper preload` then `hyprctl hyprpaper wallpaper "MONITOR,PATH"` per monitor. We require `hyprpaper` running; we do not start it.
- **Sway/wlroots.** Prefer `swww` (smooth fades), fall back to `swaybg`. `swww img --outputs DP-1 path.png` for per-monitor.
- **feh.** `feh --bg-fill IMAGE1 IMAGE2 …` — feh handles per-monitor compositing.
- **Custom.** Shell command template from config with `{image_N}` placeholders. Runs with the same subprocess rules; user is responsible for command safety.

## 10.5 Backend feature flags

Some backends pull weight (zbus is ~1 MB compiled). We gate them behind Cargo features (`backend-kde`, `backend-gnome`, …) all on by default; minimal-distro packagers can disable some.
