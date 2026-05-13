//! X11 `feh` backend via `feh --bg-fill IMAGE…`.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Instant;

use tracing::{debug, info};

use crate::display::{Availability, MonitorRef};

use super::subprocess::{DEFAULT_TIMEOUT, run, which};
use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "feh";
const TOOL: &str = "feh";

#[derive(Debug, Default)]
pub struct FehBackend;

impl FehBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for FehBackend {
    // reason: trait method signature is `&str`; the constant is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        NAME
    }

    fn availability(&self) -> Availability {
        if std::env::var_os("DISPLAY").is_none() {
            return Availability::WrongEnvironment {
                reason: "$DISPLAY is not set (feh requires X11)",
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
        let bg_fill = OsStr::new("--bg-fill");
        let mut args: Vec<&OsStr> = Vec::with_capacity(assignments.len() + 1);
        args.push(bg_fill);
        for (_, path) in assignments {
            args.push(path.as_os_str());
        }
        debug!(monitors = assignments.len(), backend = NAME, "spawning feh");
        let started = Instant::now();
        run(TOOL, &args, DEFAULT_TIMEOUT)?;
        let duration = started.elapsed();
        info!(monitors = assignments.len(), backend = NAME, "applied");
        Ok(AppliedReport {
            monitors_set: assignments.len(),
            duration,
            backend: NAME,
        })
    }

    fn supports_per_monitor(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)] // reason: tests fail loudly on infallible paths
mod tests {
    use super::*;

    #[test]
    fn name_is_stable() {
        assert_eq!(FehBackend::new().name(), "feh");
    }

    #[test]
    fn supports_per_monitor_is_false() {
        assert!(!FehBackend::new().supports_per_monitor());
    }

    #[test]
    fn empty_apply_returns_zero_without_spawning() {
        // Arrange
        let backend = FehBackend::new();

        // Act — must short-circuit before the availability check.
        let report = backend.apply(&[]).expect("empty apply is infallible");

        // Assert
        assert_eq!(report.monitors_set, 0);
        assert_eq!(report.backend, "feh");
    }
}
