//! User-supplied custom backend (`SPEC.md` §10.4).
//!
//! Reads a command template from `[backend].custom_command` and runs it
//! after substituting `{image_N}` and `{monitor_N}` placeholders (1-indexed).
//! The template is split on whitespace into `argv` — no shell, no quoting —
//! and each argument that *exactly equals* a placeholder is replaced by the
//! corresponding [`OsStr`]. This means paths are passed as separate args,
//! never interpolated into a string, even if the path contains spaces.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Instant;

use tracing::{debug, info};

use crate::display::{Availability, MonitorRef};

use super::subprocess::{DEFAULT_TIMEOUT, run};
use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "custom";

/// `WallpaperBackend` driven by a user-provided command template.
///
/// The template comes from [`crate::config::BackendConfig::custom_command`].
/// `{image_N}` and `{monitor_N}` placeholders (1-indexed) are replaced
/// with each pair's path / monitor name; non-placeholder tokens pass
/// through verbatim. The first token is the program; the rest are args.
#[derive(Debug, Clone)]
pub struct CustomBackend {
    template: String,
}

impl CustomBackend {
    /// Construct a `CustomBackend` from the user's configured template.
    #[must_use]
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
        }
    }
}

impl WallpaperBackend for CustomBackend {
    // reason: trait method signature is `&str`; the constant is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        NAME
    }

    fn availability(&self) -> Availability {
        if self.template.trim().is_empty() {
            return Availability::WrongEnvironment {
                reason: "[backend].custom_command is empty",
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
        let avail = self.availability();
        if avail != Availability::Available {
            return Err(BackendError::Unavailable {
                backend: NAME,
                reason: format!("availability check returned {avail:?}"),
            });
        }
        let started = Instant::now();
        let argv = build_argv(&self.template, assignments)?;
        let (program, args_owned) =
            argv.split_first()
                .ok_or_else(|| BackendError::Unavailable {
                    backend: NAME,
                    reason: "[backend].custom_command is empty after splitting".to_owned(),
                })?;
        let program_str = program.to_str().ok_or_else(|| {
            BackendError::Encode("non-UTF-8 program in custom_command".to_owned())
        })?;
        let args_ref: Vec<&OsStr> = args_owned.iter().map(OsString::as_os_str).collect();
        debug!(
            program = program_str,
            args = args_ref.len(),
            monitors = assignments.len(),
            backend = NAME,
            "spawning custom command"
        );
        run(program_str, &args_ref, DEFAULT_TIMEOUT)?;
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

/// Split the template on whitespace, then substitute placeholders.
///
/// Returns the resolved `argv` as `OsString`s so paths with non-UTF-8
/// bytes (rare on Linux but possible) survive unchanged.
pub(crate) fn build_argv(
    template: &str,
    assignments: &[(MonitorRef, PathBuf)],
) -> Result<Vec<OsString>, BackendError> {
    let tokens: Vec<&str> = template.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(BackendError::Unavailable {
            backend: NAME,
            reason: "[backend].custom_command is empty after splitting".to_owned(),
        });
    }
    let mut argv: Vec<OsString> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        if let Some(idx) = parse_placeholder(tok, "image_") {
            let (_, path) = pick(assignments, idx, "image")?;
            argv.push(path.as_os_str().to_os_string());
        } else if let Some(idx) = parse_placeholder(tok, "monitor_") {
            let (monitor, _) = pick(assignments, idx, "monitor")?;
            argv.push(OsString::from(&monitor.name));
        } else {
            argv.push(OsString::from(tok));
        }
    }
    Ok(argv)
}

fn parse_placeholder(token: &str, prefix: &str) -> Option<usize> {
    let inner = token.strip_prefix('{')?.strip_suffix('}')?;
    let n = inner.strip_prefix(prefix)?;
    n.parse::<usize>().ok().filter(|n| *n >= 1)
}

fn pick<'a>(
    assignments: &'a [(MonitorRef, PathBuf)],
    one_based_index: usize,
    kind: &'static str,
) -> Result<&'a (MonitorRef, PathBuf), BackendError> {
    assignments
        .get(one_based_index - 1)
        .ok_or_else(|| BackendError::Unavailable {
            backend: NAME,
            reason: format!(
                "custom_command references {{{kind}_{one_based_index}}} but only {} monitor(s) were provided",
                assignments.len()
            ),
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on malformed templates
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
    fn build_argv_substitutes_image_and_monitor_placeholders() {
        let pairs = vec![pair("DP-1", "/walls/a.png"), pair("DP-2", "/walls/b.png")];
        let argv = build_argv("set --to {monitor_1} {image_1}", &pairs).unwrap();
        assert_eq!(
            argv,
            vec![
                OsString::from("set"),
                OsString::from("--to"),
                OsString::from("DP-1"),
                OsString::from("/walls/a.png"),
            ]
        );
    }

    #[test]
    fn build_argv_supports_higher_indices() {
        let pairs = vec![pair("DP-1", "/a.png"), pair("DP-2", "/b.png")];
        let argv = build_argv("tool {image_2} on {monitor_2}", &pairs).unwrap();
        assert_eq!(argv[1], OsString::from("/b.png"));
        assert_eq!(argv[3], OsString::from("DP-2"));
    }

    #[test]
    fn build_argv_passes_through_unrecognised_tokens() {
        let pairs = vec![pair("DP-1", "/a.png")];
        let argv = build_argv("echo hello {image_1}", &pairs).unwrap();
        assert_eq!(argv[0], OsString::from("echo"));
        assert_eq!(argv[1], OsString::from("hello"));
        assert_eq!(argv[2], OsString::from("/a.png"));
    }

    #[test]
    fn build_argv_rejects_index_beyond_assignments() {
        let pairs = vec![pair("DP-1", "/a.png")];
        let err = build_argv("tool {image_2}", &pairs).unwrap_err();
        assert!(
            matches!(err, BackendError::Unavailable { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn build_argv_rejects_empty_template() {
        let pairs = vec![pair("DP-1", "/a.png")];
        let err = build_argv("   ", &pairs).unwrap_err();
        assert!(matches!(err, BackendError::Unavailable { .. }));
    }

    #[test]
    fn parse_placeholder_rejects_zero_and_non_numeric() {
        assert_eq!(parse_placeholder("{image_0}", "image_"), None);
        assert_eq!(parse_placeholder("{image_x}", "image_"), None);
        assert_eq!(parse_placeholder("image_1", "image_"), None);
        assert_eq!(parse_placeholder("{image_1}", "image_"), Some(1));
    }

    #[test]
    fn empty_apply_short_circuits() {
        let backend = CustomBackend::new("noop {image_1}");
        let report = backend.apply(&[]).unwrap();
        assert_eq!(report.monitors_set, 0);
    }

    #[test]
    fn empty_template_is_not_available() {
        let backend = CustomBackend::new("");
        assert!(matches!(
            backend.availability(),
            Availability::WrongEnvironment { .. }
        ));
    }

    #[test]
    fn name_and_per_monitor_flags() {
        let backend = CustomBackend::new("x");
        assert_eq!(backend.name(), "custom");
        assert!(backend.supports_per_monitor());
    }
}
