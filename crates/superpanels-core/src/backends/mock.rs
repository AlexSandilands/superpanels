//! Test-only `WallpaperBackend` that records every `apply` call.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crate::display::{Availability, MonitorRef};

use super::{AppliedReport, BackendError, WallpaperBackend};

#[derive(Debug, Default)]
pub struct MockBackend {
    applied: Mutex<Vec<(MonitorRef, PathBuf)>>,
}

impl MockBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of recorded `apply` calls.
    #[must_use]
    pub fn recorded(&self) -> Vec<(MonitorRef, PathBuf)> {
        // reason: poisoning would mean a previous test panicked while
        // holding the lock; surfacing the inner data is still correct.
        #[allow(clippy::expect_used)]
        self.applied
            .lock()
            .expect("MockBackend mutex poisoned by an earlier test panic")
            .clone()
    }
}

impl WallpaperBackend for MockBackend {
    // reason: trait method signature is `&str`; the literal is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "mock"
    }

    fn availability(&self) -> Availability {
        Availability::Available
    }

    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError> {
        // reason: poisoning would mean a previous test panicked while
        // holding the lock; recording is still correct after recovery.
        #[allow(clippy::expect_used)]
        let mut guard = self
            .applied
            .lock()
            .expect("MockBackend mutex poisoned by an earlier test panic");
        guard.extend(assignments.iter().cloned());
        Ok(AppliedReport {
            monitors_set: assignments.len(),
            duration: Duration::ZERO,
            backend: "mock",
        })
    }

    fn supports_per_monitor(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: MockBackend's apply is infallible; tests fail loudly if not
mod tests {
    use super::*;

    fn pair(name: &str, path: &str) -> (MonitorRef, PathBuf) {
        (
            MonitorRef {
                stable_id: format!("{name}-id"),
                name: name.to_owned(),
            },
            PathBuf::from(path),
        )
    }

    #[test]
    fn empty_apply_records_nothing_and_reports_zero() {
        // Arrange
        let backend = MockBackend::new();

        // Act
        let report = backend.apply(&[]).unwrap();

        // Assert
        assert_eq!(report.monitors_set, 0);
        assert_eq!(report.backend, "mock");
        assert!(backend.recorded().is_empty());
    }

    #[test]
    fn apply_records_each_pair_in_order() {
        // Arrange
        let backend = MockBackend::new();
        let pairs = vec![pair("DP-1", "/tmp/a.png"), pair("DP-2", "/tmp/b.png")];

        // Act
        let report = backend.apply(&pairs).unwrap();

        // Assert
        assert_eq!(report.monitors_set, 2);
        let recorded = backend.recorded();
        assert_eq!(recorded, pairs);
    }

    #[test]
    fn availability_is_always_available() {
        // Arrange + Act
        let avail = MockBackend::new().availability();

        // Assert
        assert_eq!(avail, Availability::Available);
    }
}
