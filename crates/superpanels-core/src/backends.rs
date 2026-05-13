//! Wallpaper backends.

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

/// Outcome of a successful [`WallpaperBackend::apply`]. For composite
/// backends (e.g. GNOME), `monitors_set` is the count of monitors covered,
/// not the count of subprocesses invoked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedReport {
    pub monitors_set: usize,
    pub duration: Duration,
    pub backend: &'static str,
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("backend `{backend}` is not available: {reason}")]
    Unavailable {
        backend: &'static str,
        reason: String,
    },
    #[error("subprocess `{cmd}` failed (exit {exit}): {stderr}")]
    Subprocess {
        cmd: String,
        exit: i32,
        stderr: String,
    },
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout { cmd: String, seconds: u64 },
    #[error("D-Bus call failed: {0}")]
    DBus(String),
    #[error("monitor `{0}` not present in current layout")]
    UnknownMonitor(String),
    /// Surfaced rather than `unwrap`'d — paths *are* valid JSON strings, but
    /// we don't want a panic if that ever stops being true.
    #[error("could not encode payload: {0}")]
    Encode(String),
}

/// `Send + Sync` so the daemon can hold one in an `Arc` and dispatch from
/// multiple threads without a `Mutex`.
pub trait WallpaperBackend: Send + Sync {
    fn name(&self) -> &str;
    /// Cheap check — no subprocess spawn.
    fn availability(&self) -> Availability;
    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError>;
    /// `false` for backends that composite then set one image.
    fn supports_per_monitor(&self) -> bool;
}

/// Walk the ladder and return the first available backend.
/// A non-`Auto` `prefer` is honoured unconditionally — the apply-time error
/// is more actionable than a silent fallback. When `Auto` finds nothing, an
/// [`UnavailableBackend`] sentinel is returned.
#[must_use]
pub fn detect_backend(prefer: BackendKind, custom_command: &str) -> Box<dyn WallpaperBackend> {
    if prefer != BackendKind::Auto {
        debug!(?prefer, "backend pinned via config");
        return construct(prefer, custom_command);
    }
    // Order matches priority table.
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

/// Sentinel returned by [`detect_backend`] when nothing is available;
/// `apply()` errors with the list of backends that were tried.
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
