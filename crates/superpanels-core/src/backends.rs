//! Wallpaper backends: the [`WallpaperBackend`] trait and its impls.
//!
//! Mirrors `SPEC.md` §10.1. Each compositor backend is a separate
//! submodule; the trait definition + shared types live here. [`MockBackend`]
//! is feature-free (always compiled) so any test in the workspace can
//! exercise the trait contract without bringing up a real desktop.
//!
//! [`detect_backend`] walks the `SPEC.md` §10.2 priority ladder and
//! returns the first available backend, honouring an explicit
//! [`crate::config::BackendKind`] pin from config.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;
use tracing::debug;

use crate::config::BackendKind;
use crate::display::{Availability, MonitorRef};

pub mod custom;
pub mod feh;
pub mod gnome;
pub mod hyprland;
pub mod kde;
pub mod mock;
pub(crate) mod subprocess;
pub mod sway;

pub use custom::CustomBackend;
pub use feh::FehBackend;
pub use gnome::GnomeBackend;
pub use hyprland::HyprlandBackend;
pub use kde::KdeBackend;
pub use mock::MockBackend;
pub use sway::SwayBackend;

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

/// Walk the `SPEC.md` §10.2 ladder and return the first available backend.
///
/// If `prefer != BackendKind::Auto`, the ladder is skipped and the pinned
/// backend is returned regardless of availability — callers see the
/// availability error from `apply()` rather than a silent fallback. When
/// `Auto` is requested and no backend is available, an
/// [`UnavailableBackend`] is returned whose `apply()` always errors with a
/// message listing what was tried.
///
/// `custom_command` is the user-configured template from
/// `[backend].custom_command` and is consumed only when the resolved
/// backend is [`BackendKind::Custom`] or when the ladder runs and reaches
/// the custom slot.
#[must_use]
pub fn detect_backend(prefer: BackendKind, custom_command: &str) -> Box<dyn WallpaperBackend> {
    if prefer != BackendKind::Auto {
        debug!(?prefer, "backend pinned via config");
        return construct(prefer, custom_command);
    }
    // Order matches SPEC §10.2 priority table.
    let ladder: &[BackendKind] = &[
        BackendKind::Kde,
        BackendKind::Hyprland,
        BackendKind::Sway,
        BackendKind::Gnome,
        BackendKind::Feh,
        BackendKind::Custom,
    ];
    let mut tried: Vec<&'static str> = Vec::with_capacity(ladder.len());
    for kind in ladder {
        let backend = construct(*kind, custom_command);
        let avail = backend.availability();
        if avail == Availability::Available {
            debug!(backend = backend.name(), "auto-detected available backend");
            return backend;
        }
        tried.push(static_name(*kind));
    }
    Box::new(UnavailableBackend { tried })
}

fn construct(kind: BackendKind, custom_command: &str) -> Box<dyn WallpaperBackend> {
    match kind {
        BackendKind::Auto | BackendKind::Kde => Box::new(KdeBackend::new()),
        BackendKind::Gnome => Box::new(GnomeBackend::new()),
        BackendKind::Sway => Box::new(SwayBackend::new()),
        BackendKind::Hyprland => Box::new(HyprlandBackend::new()),
        BackendKind::Feh => Box::new(FehBackend::new()),
        BackendKind::Custom => Box::new(CustomBackend::new(custom_command.to_owned())),
    }
}

fn static_name(kind: BackendKind) -> &'static str {
    match kind {
        BackendKind::Auto | BackendKind::Kde => "kde",
        BackendKind::Gnome => "gnome",
        BackendKind::Sway => "sway",
        BackendKind::Hyprland => "hyprland",
        BackendKind::Feh => "feh",
        BackendKind::Custom => "custom",
    }
}

/// Sentinel backend returned by [`detect_backend`] when every backend in
/// the ladder reports unavailable.
///
/// `apply()` always errors with a [`BackendError::Unavailable`] that lists
/// the backends that were tried, so the CLI / GUI can surface a single
/// actionable message instead of silently doing nothing.
#[derive(Debug)]
pub struct UnavailableBackend {
    tried: Vec<&'static str>,
}

impl WallpaperBackend for UnavailableBackend {
    // reason: trait method signature is `&str`; the literal is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "none"
    }

    fn availability(&self) -> Availability {
        Availability::WrongEnvironment {
            reason: "no wallpaper backend is available on this system",
        }
    }

    fn apply(&self, _assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError> {
        Err(BackendError::Unavailable {
            backend: "none",
            reason: format!(
                "no wallpaper backend is available; tried: {}. \
                 Pin one with `[backend].prefer = \"…\"` or install a supported tool.",
                self.tried.join(", ")
            ),
        })
    }

    fn supports_per_monitor(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on detection bugs
mod tests {
    use super::*;

    #[test]
    fn pinned_kind_is_returned_unconditionally() {
        let backend = detect_backend(BackendKind::Custom, "echo {image_1}");
        assert_eq!(backend.name(), "custom");
    }

    #[test]
    fn pinned_feh_returns_feh_even_without_display() {
        // We're not asserting availability — only that the pin is
        // honoured. apply() against a non-X11 host would error, which is
        // the documented behaviour.
        let backend = detect_backend(BackendKind::Feh, "");
        assert_eq!(backend.name(), "feh");
    }

    #[test]
    fn auto_detects_some_backend_or_returns_sentinel() {
        // Whatever the host happens to have, detect_backend must not panic
        // and must return a Box<dyn WallpaperBackend>. If the sentinel is
        // returned, its apply() must error with Unavailable.
        let backend = detect_backend(BackendKind::Auto, "");
        let report = backend.apply(&[]);
        // Empty assignments short-circuit even on the sentinel? No — the
        // sentinel always errors; real backends short-circuit on empty.
        if backend.name() == "none" {
            assert!(matches!(report, Err(BackendError::Unavailable { .. })));
        } else {
            assert!(
                report.is_ok(),
                "real backend should accept empty apply: {report:?}"
            );
        }
    }

    #[test]
    fn unavailable_backend_apply_lists_tried_backends() {
        let sentinel = UnavailableBackend {
            tried: vec!["kde", "gnome"],
        };
        let err = sentinel.apply(&[]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("kde"), "msg was: {msg}");
        assert!(msg.contains("gnome"), "msg was: {msg}");
    }
}
