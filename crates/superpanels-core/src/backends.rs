//! Wallpaper backends: the [`WallpaperBackend`] trait and its impls.
//!
//! Mirrors `SPEC.md` §10.1. Each compositor backend is a separate
//! submodule; the trait definition + shared types live here. [`MockBackend`]
//! is feature-free (always compiled) so any test in the workspace can
//! exercise the trait contract without bringing up a real desktop.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use crate::display::{Availability, MonitorRef};

pub mod kde;
pub mod mock;

pub use kde::KdeBackend;
pub use mock::MockBackend;

/// Outcome of a successful [`WallpaperBackend::apply`].
///
/// Returned even on partial successes (e.g. a composite-style backend
/// touched 1 image but covered 3 monitors); use `monitors_set` to count
/// monitors, not subprocess invocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedReport {
    /// Number of monitors successfully assigned a wallpaper. For composite
    /// backends (GNOME) this is the count of monitors covered by the composite.
    pub monitors_set: usize,
    /// Wall-clock duration of the apply, including any subprocess + redraw
    /// wait.
    pub duration: Duration,
    /// Backend name that handled the apply (for diagnostics).
    pub backend: &'static str,
}

/// Errors any backend can return from [`WallpaperBackend::apply`].
///
/// Variants mirror `SPEC.md` §10.1 so the CLI / GUI can react specifically
/// (toast colour, suggested fix) rather than printing one opaque string.
#[derive(Debug, Error)]
pub enum BackendError {
    /// `availability` returned a non-`Available` variant; the apply was
    /// refused before any subprocess / D-Bus traffic.
    #[error("backend `{backend}` is not available: {reason}")]
    Unavailable {
        /// Backend name (e.g. `"kde"`).
        backend: &'static str,
        /// Human-readable reason from the availability check.
        reason: String,
    },
    /// A subprocess exited non-zero.
    #[error("subprocess `{cmd}` failed (exit {exit}): {stderr}")]
    Subprocess {
        /// Command line.
        cmd: String,
        /// Exit code.
        exit: i32,
        /// Captured stderr.
        stderr: String,
    },
    /// A subprocess didn't return within the per-backend timeout.
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout {
        /// Command line.
        cmd: String,
        /// Configured timeout in seconds.
        seconds: u64,
    },
    /// A D-Bus call failed.
    #[error("D-Bus call failed: {0}")]
    DBus(String),
    /// `apply` was given a `MonitorRef` whose `name` / `stable_id` is not
    /// in the current layout.
    #[error("monitor `{0}` not present in current layout")]
    UnknownMonitor(String),
    /// JSON serialisation of an injected payload (e.g. the KDE template
    /// arguments) failed. Should not happen in practice — paths are valid
    /// JSON strings — but we surface it rather than `unwrap`.
    #[error("could not encode payload: {0}")]
    Encode(String),
}

/// Apply per-monitor wallpapers via whatever the host compositor expects.
///
/// Implementations are stateless and `Send + Sync` so the daemon can hold
/// one in an `Arc` and dispatch from multiple threads without a `Mutex`.
pub trait WallpaperBackend: Send + Sync {
    /// Short, stable identifier (`"kde"`, `"mock"`, …). Used in
    /// [`AppliedReport::backend`] and `superpanels detect --debug`.
    fn name(&self) -> &str;
    /// Cheap, non-spawning availability check. Mirrors the
    /// [`crate::DisplayDetector`] convention of returning a rich enum so
    /// the caller can explain *why* a backend was skipped.
    fn availability(&self) -> Availability;
    /// Apply each `(monitor, image)` pair.
    ///
    /// # Errors
    ///
    /// Returns the relevant [`BackendError`] variant when the apply fails.
    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError>;
    /// Whether this backend can set a different wallpaper per monitor.
    /// `false` for backends that have to composite then set one image.
    fn supports_per_monitor(&self) -> bool;
}
