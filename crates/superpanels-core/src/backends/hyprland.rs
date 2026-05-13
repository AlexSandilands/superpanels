//! Hyprland backend via `hyprctl hyprpaper`.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Instant;

use tracing::{debug, info};

use crate::display::{Availability, MonitorRef};

use super::subprocess::{DEFAULT_TIMEOUT, run, which};
use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "hyprland";
const TOOL: &str = "hyprctl";
const START_HINT: &str = "start it with `hyprpaper &` (or via your hyprland.conf `exec-once`)";

#[derive(Debug, Default)]
pub struct HyprlandBackend;

impl HyprlandBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for HyprlandBackend {
    // reason: trait method signature is `&str`; the constant is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        NAME
    }

    fn availability(&self) -> Availability {
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            return Availability::WrongEnvironment {
                reason: "$HYPRLAND_INSTANCE_SIGNATURE is not set",
            };
        }
        if !which(TOOL) {
            return Availability::ToolMissing { tool: TOOL };
        }
        Availability::Available
    }

    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError> {
        if assignments.is_empty() {
            return Ok(AppliedReport {
                monitors_set: 0,
                duration: std::time::Duration::ZERO,
                backend: NAME,
            });
        }
        let avail = self.availability();
        if avail != Availability::Available {
            return Err(BackendError::Unavailable {
                backend: NAME,
                reason: format!("availability check returned {avail:?}"),
            });
        }
        let started = Instant::now();
        for (monitor, path) in assignments {
            preload(path)?;
            set_wallpaper(monitor, path)?;
            debug!(monitor = %monitor.name, backend = NAME, "wallpaper set");
        }
        let duration = started.elapsed();
        info!(monitors = assignments.len(), backend = NAME, "applied");
        Ok(AppliedReport {
            monitors_set: assignments.len(),
            duration,
            backend: NAME,
        })
    }

    fn supports_per_monitor(&self) -> bool {
        true
    }
}

fn preload(path: &std::path::Path) -> Result<(), BackendError> {
    let args: [&OsStr; 3] = [
        OsStr::new("hyprpaper"),
        OsStr::new("preload"),
        path.as_os_str(),
    ];
    run(TOOL, &args, DEFAULT_TIMEOUT)
        .map(|_| ())
        .map_err(map_unavailable_subprocess)
}

fn set_wallpaper(monitor: &MonitorRef, path: &std::path::Path) -> Result<(), BackendError> {
    let spec = format_wallpaper_spec(&monitor.name, path);
    let args: [&OsStr; 3] = [
        OsStr::new("hyprpaper"),
        OsStr::new("wallpaper"),
        spec.as_os_str(),
    ];
    run(TOOL, &args, DEFAULT_TIMEOUT)
        .map(|_| ())
        .map_err(map_unavailable_subprocess)
}

fn format_wallpaper_spec(monitor_name: &str, path: &std::path::Path) -> OsString {
    let mut out = OsString::new();
    out.push(monitor_name);
    out.push(",");
    out.push(path.as_os_str());
    out
}

/// Promote "hyprpaper not running" stderr to `Unavailable` with a start hint.
fn map_unavailable_subprocess(err: BackendError) -> BackendError {
    if let BackendError::Subprocess { stderr, .. } = &err
        && stderr_indicates_no_hyprpaper(stderr)
    {
        return BackendError::Unavailable {
            backend: NAME,
            reason: format!("hyprpaper does not appear to be running: {START_HINT}"),
        };
    }
    err
}

fn stderr_indicates_no_hyprpaper(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("hyprpaper") && (lower.contains("not running") || lower.contains("no such"))
        || lower.contains("couldn't connect")
        || lower.contains("could not connect")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on infallible paths
mod tests {
    use super::*;

    #[test]
    fn name_and_per_monitor_flags() {
        let b = HyprlandBackend::new();
        assert_eq!(b.name(), "hyprland");
        assert!(b.supports_per_monitor());
    }

    #[test]
    fn format_wallpaper_spec_joins_with_comma() {
        let spec = format_wallpaper_spec("DP-1", std::path::Path::new("/walls/a.png"));
        assert_eq!(spec, OsString::from("DP-1,/walls/a.png"));
    }

    #[test]
    fn map_unavailable_recognises_no_hyprpaper_stderr() {
        let err = BackendError::Subprocess {
            cmd: "hyprctl hyprpaper preload x".to_owned(),
            exit: 1,
            stderr: "hyprpaper is not running".to_owned(),
        };
        let mapped = map_unavailable_subprocess(err);
        assert!(matches!(
            mapped,
            BackendError::Unavailable {
                backend: "hyprland",
                ..
            }
        ));
    }

    #[test]
    fn map_unavailable_passes_through_unrelated_errors() {
        let err = BackendError::Subprocess {
            cmd: "x".into(),
            exit: 2,
            stderr: "unrelated kaboom".into(),
        };
        let mapped = map_unavailable_subprocess(err);
        assert!(matches!(mapped, BackendError::Subprocess { .. }));
    }

    #[test]
    fn empty_apply_returns_zero() {
        let report = HyprlandBackend::new().apply(&[]).unwrap();
        assert_eq!(report.monitors_set, 0);
    }
}
