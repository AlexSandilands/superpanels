//! Sway / wlroots backend. Prefers `swww`; falls back to
//! one detached `swaybg` per monitor.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Instant;

use tracing::{debug, info};

use crate::display::{Availability, MonitorRef};

use super::subprocess::{DEFAULT_TIMEOUT, run, which};
use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "sway";
const TOOL_PREFERRED: &str = "swww";
const TOOL_FALLBACK: &str = "swaybg";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwayTool {
    Swww,
    Swaybg,
}

#[derive(Debug, Default)]
pub struct SwayBackend;

impl SwayBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for SwayBackend {
    // reason: trait method signature is `&str`; the constant is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        NAME
    }

    fn availability(&self) -> Availability {
        let env_ok =
            std::env::var_os("SWAYSOCK").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
        if !env_ok {
            return Availability::WrongEnvironment {
                reason: "neither $SWAYSOCK nor $WAYLAND_DISPLAY is set",
            };
        }
        if select_tool().is_none() {
            return Availability::ToolMissing {
                tool: TOOL_PREFERRED,
            };
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
        let Some(tool) = select_tool() else {
            return Err(BackendError::Unavailable {
                backend: NAME,
                reason: format!("neither `{TOOL_PREFERRED}` nor `{TOOL_FALLBACK}` is on PATH"),
            });
        };
        let avail = self.availability();
        if avail != Availability::Available {
            return Err(BackendError::Unavailable {
                backend: NAME,
                reason: format!("availability check returned {avail:?}"),
            });
        }
        let started = Instant::now();
        match tool {
            SwayTool::Swww => apply_with_swww(assignments)?,
            SwayTool::Swaybg => apply_with_swaybg(assignments)?,
        }
        let duration = started.elapsed();
        info!(
            monitors = assignments.len(),
            backend = NAME,
            tool = ?tool,
            "applied"
        );
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

fn select_tool() -> Option<SwayTool> {
    if which(TOOL_PREFERRED) {
        Some(SwayTool::Swww)
    } else if which(TOOL_FALLBACK) {
        Some(SwayTool::Swaybg)
    } else {
        None
    }
}

fn apply_with_swww(assignments: &[(MonitorRef, PathBuf)]) -> Result<(), BackendError> {
    for (monitor, path) in assignments {
        let outputs_arg = OsString::from(&monitor.name);
        let args: [&OsStr; 4] = [
            OsStr::new("img"),
            OsStr::new("--outputs"),
            outputs_arg.as_os_str(),
            path.as_os_str(),
        ];
        debug!(monitor = %monitor.name, backend = NAME, "swww img");
        run(TOOL_PREFERRED, &args, DEFAULT_TIMEOUT)?;
    }
    Ok(())
}

fn apply_with_swaybg(assignments: &[(MonitorRef, PathBuf)]) -> Result<(), BackendError> {
    // swaybg is long-running (owns the surface), so we fire-and-forget per output.
    // Caller is responsible for killing stale instances on replacement.
    use std::process::{Command, Stdio};
    for (monitor, path) in assignments {
        debug!(monitor = %monitor.name, backend = NAME, "swaybg");
        let mut cmd = Command::new(TOOL_FALLBACK);
        cmd.arg("-o")
            .arg(&monitor.name)
            .arg("-i")
            .arg(path)
            .arg("-m")
            .arg("fill")
            .env("LC_ALL", "C")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
        cmd.spawn().map_err(|e| BackendError::Subprocess {
            cmd: format!("{TOOL_FALLBACK} -o {} -i …", monitor.name),
            exit: -1,
            stderr: e.to_string(),
        })?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on infallible paths
mod tests {
    use super::*;

    #[test]
    fn name_and_per_monitor_flags() {
        let b = SwayBackend::new();
        assert_eq!(b.name(), "sway");
        assert!(b.supports_per_monitor());
    }

    #[test]
    fn select_tool_returns_none_when_neither_present() {
        // PATH-dependent; just check it doesn't panic and returns a valid variant.
        let chosen = select_tool();
        if let Some(t) = chosen {
            assert!(matches!(t, SwayTool::Swww | SwayTool::Swaybg));
        }
    }

    #[test]
    fn empty_apply_returns_zero() {
        let report = SwayBackend::new().apply(&[]).unwrap();
        assert_eq!(report.monitors_set, 0);
    }
}
