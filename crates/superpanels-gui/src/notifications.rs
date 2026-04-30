//! `notify-rust` (`org.freedesktop.Notifications`) helper (`SPEC.md` §13.4).
//!
//! Notifications are off by default. The frontend reads the user's setting
//! from config (`general.notifications`), but apply *errors* surface
//! regardless ("errors-only" mode).

use crate::errors::IpcError;

/// Notification verbosity, parsed from `general.notifications` in the config
/// (`SPEC.md` §14.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Off,
    ErrorsOnly,
    All,
}

impl Verbosity {
    pub fn from_setting(s: &str) -> Self {
        match s {
            "all" => Self::All,
            "off" => Self::Off,
            // Treat unknown / "errors" / blank as errors-only — the spec
            // default — rather than silently swallowing apply failures.
            _ => Self::ErrorsOnly,
        }
    }

    pub fn allows_success(self) -> bool {
        matches!(self, Self::All)
    }

    pub fn allows_error(self) -> bool {
        matches!(self, Self::All | Self::ErrorsOnly)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Success,
    Error,
}

pub fn notify(
    severity: Severity,
    title: &str,
    body: &str,
    verbosity: Verbosity,
) -> Result<(), IpcError> {
    let allowed = match severity {
        Severity::Success => verbosity.allows_success(),
        Severity::Error => verbosity.allows_error(),
    };
    if !allowed {
        return Ok(());
    }
    let mut n = notify_rust::Notification::new();
    n.summary(title)
        .body(body)
        .appname("Superpanels")
        .icon("superpanels")
        .timeout(notify_rust::Timeout::Milliseconds(4000));
    n.show()
        .map(|_| ())
        .map_err(|e| IpcError::internal(format!("notify-rust: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_from_setting_maps_strings() {
        assert_eq!(Verbosity::from_setting("off"), Verbosity::Off);
        assert_eq!(Verbosity::from_setting("errors"), Verbosity::ErrorsOnly);
        assert_eq!(Verbosity::from_setting("all"), Verbosity::All);
        // Unknown defaults to errors-only — never silently off.
        assert_eq!(Verbosity::from_setting(""), Verbosity::ErrorsOnly);
        assert_eq!(Verbosity::from_setting("bogus"), Verbosity::ErrorsOnly);
    }

    #[test]
    fn off_blocks_both_severities() {
        assert!(!Verbosity::Off.allows_success());
        assert!(!Verbosity::Off.allows_error());
    }

    #[test]
    fn errors_only_allows_errors_blocks_success() {
        assert!(!Verbosity::ErrorsOnly.allows_success());
        assert!(Verbosity::ErrorsOnly.allows_error());
    }

    #[test]
    fn all_allows_both_severities() {
        assert!(Verbosity::All.allows_success());
        assert!(Verbosity::All.allows_error());
    }
}
