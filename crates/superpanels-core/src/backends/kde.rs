//! KDE Plasma backend via `org.kde.PlasmaShell.evaluateScript`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::display::{Availability, MonitorRef};

use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "kde";
const SERVICE: &str = "org.kde.plasmashell";
const PATH: &str = "/PlasmaShell";
const INTERFACE: &str = "org.kde.PlasmaShell";
const METHOD: &str = "evaluateScript";
const APPLY_TIMEOUT: Duration = Duration::from_secs(10);
const TEMPLATE_VERSION: u32 = 2;

// Plasma 6's scripting API has no `outputName` property on Desktop and no
// `screenName(int)` global, so we can't iterate desktops and look up their
// connector. Instead we iterate the assignments (keyed by connector) and
// resolve `screenForConnector(name) -> int`, then `desktopForScreen(int)`.
// A connector that isn't on the active layout returns `-1`; we skip it.
// The script `print()`s the count it actually wrote so the Rust side can
// surface partial application instead of trusting `assignments.len()`.
const SCRIPT_HEADER: &str = "// superpanels evaluateScript template v2\n\
                             var assignments = ASSIGNMENTS_LITERAL;\n\
                             var applied = 0;\n\
                             for (var name in assignments) {\n\
                                 var screen = screenForConnector(name);\n\
                                 if (screen < 0) { continue; }\n\
                                 var d = desktopForScreen(screen);\n\
                                 if (!d) { continue; }\n\
                                 d.wallpaperPlugin = 'org.kde.image';\n\
                                 d.currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General'];\n\
                                 d.writeConfig('Image', 'file://' + assignments[name]);\n\
                                 applied++;\n\
                             }\n\
                             print(applied);\n";

/// `WallpaperBackend` for KDE Plasma. A fresh D-Bus connection is opened per
/// `apply` so the daemon survives a Plasma restart without stale handles.
#[derive(Debug, Default)]
pub struct KdeBackend;

impl KdeBackend {
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
        let output = evaluate_script(&script)?;
        let duration = started.elapsed();
        let monitors_set = parse_applied_count(&output)?;
        if monitors_set != assignments.len() {
            warn!(
                requested = assignments.len(),
                applied = monitors_set,
                backend = NAME,
                "Plasma applied fewer monitors than requested (unknown connector?)",
            );
        }
        info!(monitors = monitors_set, backend = NAME, "applied");
        Ok(AppliedReport {
            monitors_set,
            duration,
            backend: NAME,
        })
    }
}

/// Pure helper: tests can call this without mutating the process env
/// (`std::env::set_var` is `unsafe` in Rust 2024).
pub(crate) fn env_indicates_kde(
    kde_full_session: Option<&str>,
    xdg_current_desktop: Option<&str>,
) -> bool {
    if kde_full_session.is_some() {
        return true;
    }
    xdg_current_desktop.is_some_and(|d| d.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
}

/// Build the JS payload. Image paths are JSON-quoted so quotes/backslashes
/// in paths can't break out of the containing string literal.
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

pub(crate) fn parse_applied_count(output: &str) -> Result<usize, BackendError> {
    let trimmed = output.trim();
    trimmed.parse::<usize>().map_err(|_| {
        BackendError::DBus(format!(
            "evaluateScript returned non-numeric output: {trimmed:?}"
        ))
    })
}

#[cfg(unix)]
fn evaluate_script(script: &str) -> Result<String, BackendError> {
    let connection =
        zbus::blocking::Connection::session().map_err(|e| BackendError::DBus(e.to_string()))?;
    // zbus's blocking call_method has no built-in timeout; enforce
    // by running the call on a worker thread and joining via recv_timeout.
    let (tx, rx) = std::sync::mpsc::channel();
    let conn = connection;
    let s = script.to_owned();
    std::thread::spawn(move || {
        let result = conn
            .call_method(Some(SERVICE), PATH, Some(INTERFACE), METHOD, &(s,))
            .map_err(|e| BackendError::DBus(e.to_string()))
            .and_then(|reply| {
                reply
                    .body()
                    .deserialize::<String>()
                    .map_err(|e| BackendError::DBus(format!("decoding reply: {e}")))
            });
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
fn evaluate_script(_script: &str) -> Result<String, BackendError> {
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

        // Assert
        assert!(
            script.contains("\"DP-1\":\"/walls/a.png\"")
                || script.contains("\"DP-1\": \"/walls/a.png\"")
        );
        assert!(
            script.contains("\"DP-2\":\"/walls/b.png\"")
                || script.contains("\"DP-2\": \"/walls/b.png\"")
        );
        assert!(script.contains("evaluateScript template v2"));
        assert!(script.contains("screenForConnector"));
        assert!(script.contains("desktopForScreen"));
    }

    #[test]
    fn build_script_escapes_special_characters_in_path() {
        // Arrange
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

        // Assert
        assert!(script.contains("\\\"quote\\\""));
        assert!(script.contains("\\\\and\\\\backslash"));
    }

    #[test]
    fn availability_decides_purely_from_env_string_inputs() {
        assert!(env_indicates_kde(Some(""), Some("KDE")));
        assert!(env_indicates_kde(Some("true"), None));
        assert!(env_indicates_kde(None, Some("X-Cinnamon:KDE")));
        assert!(!env_indicates_kde(None, Some("GNOME")));
        assert!(!env_indicates_kde(None, None));
    }

    #[test]
    fn parse_applied_count_accepts_trimmed_integer() {
        assert_eq!(parse_applied_count("3").unwrap(), 3);
        assert_eq!(parse_applied_count("  0\n").unwrap(), 0);
    }

    #[test]
    fn parse_applied_count_rejects_non_numeric_output() {
        let err = parse_applied_count("TypeError: undefined").unwrap_err();
        assert!(matches!(err, BackendError::DBus(_)), "got {err:?}");
    }

    #[test]
    #[ignore = "requires a live KDE Plasma session bus"]
    fn apply_against_live_session_succeeds() {
        let pairs = vec![pair("DP-1", "/tmp/superpanels-smoke.png")];
        let result = KdeBackend::new().apply(&pairs);
        assert!(result.is_ok(), "apply failed: {result:?}");
    }
}
