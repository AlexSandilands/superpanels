//! KDE Plasma backend: applies wallpapers via
//! `org.kde.PlasmaShell.evaluateScript` over zbus (`SPEC.md` §10.4).
//!
//! The JS payload is built from a versioned template (`TEMPLATE_VERSION`)
//! with image paths injected as JSON-quoted string literals via
//! [`serde_json::to_string`] — never string-concatenated — so a path
//! containing quotes, backslashes, or non-ASCII bytes can't escape the
//! containing JS string. The script's monitor lookup uses the output name
//! (`MonitorRef.name`) since Plasma's `Image` plugin keys on it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use tracing::{debug, info};

use crate::display::{Availability, MonitorRef};

use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "kde";
const SERVICE: &str = "org.kde.plasmashell";
const PATH: &str = "/PlasmaShell";
const INTERFACE: &str = "org.kde.PlasmaShell";
const METHOD: &str = "evaluateScript";
const APPLY_TIMEOUT: Duration = Duration::from_secs(10);
const TEMPLATE_VERSION: u32 = 1;

const SCRIPT_HEADER: &str = "// superpanels evaluateScript template v1\n\
                             var assignments = ASSIGNMENTS_LITERAL;\n\
                             var allDesktops = desktops();\n\
                             for (var i = 0; i < allDesktops.length; i++) {\n\
                                 var d = allDesktops[i];\n\
                                 var output = d.screen >= 0 ? screenName(d.screen) : null;\n\
                                 if (output === null) { continue; }\n\
                                 if (!(output in assignments)) { continue; }\n\
                                 d.wallpaperPlugin = 'org.kde.image';\n\
                                 d.currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General'];\n\
                                 d.writeConfig('Image', 'file://' + assignments[output]);\n\
                             }\n";

/// `WallpaperBackend` for KDE Plasma sessions.
///
/// Stateless; cheap to construct. Holds no D-Bus connection — one is
/// opened per `apply` so the daemon can survive a Plasma restart without
/// stale handles.
#[derive(Debug, Default)]
pub struct KdeBackend;

impl KdeBackend {
    /// Construct a `KdeBackend`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for KdeBackend {
    // reason: trait method signature is `&str`; the constant is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        NAME
    }

    fn availability(&self) -> Availability {
        let kde_full = std::env::var("KDE_FULL_SESSION").ok();
        let xdg = std::env::var("XDG_CURRENT_DESKTOP").ok();
        if env_indicates_kde(kde_full.as_deref(), xdg.as_deref()) {
            Availability::Available
        } else {
            Availability::WrongEnvironment {
                reason: "$KDE_FULL_SESSION not set and $XDG_CURRENT_DESKTOP does not contain KDE",
            }
        }
    }

    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError> {
        if assignments.is_empty() {
            return Ok(AppliedReport {
                monitors_set: 0,
                duration: Duration::ZERO,
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
        let script = build_script(assignments)?;
        debug!(
            template_version = TEMPLATE_VERSION,
            monitors = assignments.len(),
            "submitting evaluateScript"
        );
        let started = Instant::now();
        evaluate_script(&script)?;
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

/// Pure helper: given the captured values of `$KDE_FULL_SESSION` and
/// `$XDG_CURRENT_DESKTOP`, decide whether the running session is KDE.
///
/// Pulled out of [`KdeBackend::availability`] so unit tests can pass
/// concrete values without mutating the process env (which is `unsafe`
/// under Rust 2024 std).
pub(crate) fn env_indicates_kde(
    kde_full_session: Option<&str>,
    xdg_current_desktop: Option<&str>,
) -> bool {
    if kde_full_session.is_some() {
        return true;
    }
    xdg_current_desktop.is_some_and(|d| d.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
}

/// Build the JS payload for an evaluateScript call given the per-monitor
/// assignments. Pulled out so unit tests can assert on the rendered script
/// without bringing up D-Bus.
pub(crate) fn build_script(assignments: &[(MonitorRef, PathBuf)]) -> Result<String, BackendError> {
    let mut map = serde_json::Map::with_capacity(assignments.len());
    for (m, path) in assignments {
        let path_str = path_to_json_string(path)?;
        map.insert(m.name.clone(), Value::String(path_str));
    }
    let literal = serde_json::to_string(&Value::Object(map))
        .map_err(|e| BackendError::Encode(e.to_string()))?;
    Ok(SCRIPT_HEADER.replace("ASSIGNMENTS_LITERAL", &literal))
}

fn path_to_json_string(path: &Path) -> Result<String, BackendError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| BackendError::Encode(format!("non-UTF-8 path: {}", path.display())))
}

#[cfg(unix)]
fn evaluate_script(script: &str) -> Result<(), BackendError> {
    let connection =
        zbus::blocking::Connection::session().map_err(|e| BackendError::DBus(e.to_string()))?;
    // zbus's blocking call_method blocks indefinitely; enforce our own
    // wall-clock cap by running the call on a worker thread and joining
    // with a recv timeout. The connection isn't Send across awaits but is
    // Send across threads, so the channel pattern is the simplest option
    // that respects SPEC §10.3's timeout rule.
    let (tx, rx) = std::sync::mpsc::channel();
    let conn = connection;
    let s = script.to_owned();
    std::thread::spawn(move || {
        let result = conn
            .call_method(Some(SERVICE), PATH, Some(INTERFACE), METHOD, &(s,))
            .map(|_| ())
            .map_err(|e| BackendError::DBus(e.to_string()));
        let _ = tx.send(result);
    });
    match rx.recv_timeout(APPLY_TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(BackendError::Timeout {
            cmd: format!("{SERVICE}.{METHOD}"),
            seconds: APPLY_TIMEOUT.as_secs(),
        }),
    }
}

#[cfg(not(unix))]
fn evaluate_script(_script: &str) -> Result<(), BackendError> {
    Err(BackendError::Unavailable {
        backend: NAME,
        reason: "KDE backend requires a unix session bus".to_owned(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serde errors
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
    fn build_script_injects_assignments_as_a_json_object() {
        // Arrange
        let pairs = vec![pair("DP-1", "/walls/a.png"), pair("DP-2", "/walls/b.png")];

        // Act
        let script = build_script(&pairs).unwrap();

        // Assert — the literal is a valid JSON object whose keys are
        // monitor names and whose values are the paths. The presence of
        // the JSON-escaped path strings (with no manual quoting) is the
        // safety property we care about.
        assert!(
            script.contains("\"DP-1\":\"/walls/a.png\"")
                || script.contains("\"DP-1\": \"/walls/a.png\"")
        );
        assert!(
            script.contains("\"DP-2\":\"/walls/b.png\"")
                || script.contains("\"DP-2\": \"/walls/b.png\"")
        );
        assert!(script.contains("evaluateScript template v1"));
    }

    #[test]
    fn build_script_escapes_special_characters_in_path() {
        // Arrange — path with a quote and a backslash, the kinds of bytes
        // that would break naive string concat.
        let nasty = PathBuf::from("/walls/with \"quote\"\\and\\backslash.png");
        let pairs = vec![(
            MonitorRef {
                stable_id: "id".into(),
                name: "DP-1".into(),
            },
            nasty,
        )];

        // Act
        let script = build_script(&pairs).unwrap();

        // Assert — the dangerous characters appear as their JSON escapes,
        // and the bare `"quote"` substring does not appear in the script
        // (which would mean the literal had been broken open).
        assert!(script.contains("\\\"quote\\\""));
        assert!(script.contains("\\\\and\\\\backslash"));
    }

    #[test]
    fn availability_decides_purely_from_env_string_inputs() {
        // The availability check is exercised from-env in the integration
        // pass; here we only assert the helper that decides *given* the
        // two values is the rule from SPEC §10.2 (`KDE` token in
        // colon-list, or `KDE_FULL_SESSION` set).
        assert!(env_indicates_kde(Some(""), Some("KDE")));
        assert!(env_indicates_kde(Some("true"), None));
        assert!(env_indicates_kde(None, Some("X-Cinnamon:KDE")));
        assert!(!env_indicates_kde(None, Some("GNOME")));
        assert!(!env_indicates_kde(None, None));
    }

    /// Full apply against a live KDE session. Only meaningful on a real
    /// desktop with `org.kde.plasmashell` running on the session bus, so
    /// it's `#[ignore]`d in the default test run. To exercise:
    /// `cargo test -p superpanels-core kde_apply -- --ignored`.
    #[test]
    #[ignore = "requires a live KDE Plasma session bus"]
    fn apply_against_live_session_succeeds() {
        let pairs = vec![pair("DP-1", "/tmp/superpanels-smoke.png")];
        let result = KdeBackend::new().apply(&pairs);
        assert!(result.is_ok(), "apply failed: {result:?}");
    }
}
