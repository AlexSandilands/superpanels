//! Shared subprocess helper for display detectors.
//!
//! `SPEC.md` §6 mandates a 5-second wall-clock timeout on every detector
//! tool, plus `LC_ALL=C` so parsers see locale-independent output. The
//! `NO_COLOR=1` env is added unconditionally — text-output tools strip ANSI
//! when it is set, and JSON-output tools ignore it.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::DetectError;

/// Spawn `program` with `args`, wait up to `timeout`, capture stdout and
/// return it on a clean exit. `cmd_display` is used in error messages so
/// callers can format a stable diagnostic string without re-quoting args.
pub(crate) fn run(
    program: &str,
    args: &[&OsStr],
    timeout: Duration,
    cmd_display: &str,
) -> Result<String, DetectError> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DetectError::Subprocess {
            cmd: cmd_display.to_owned(),
            stderr: e.to_string(),
        })?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| DetectError::Subprocess {
                        cmd: cmd_display.to_owned(),
                        stderr: e.to_string(),
                    })?;
                if !status.success() {
                    return Err(DetectError::Subprocess {
                        cmd: cmd_display.to_owned(),
                        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                    });
                }
                return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    return Err(DetectError::Timeout {
                        cmd: cmd_display.to_owned(),
                        seconds: timeout.as_secs(),
                    });
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                return Err(DetectError::Subprocess {
                    cmd: cmd_display.to_owned(),
                    stderr: e.to_string(),
                });
            }
        }
    }
}

/// Look up `bin` on `$PATH`. Returns `Some(absolute path)` on the first
/// regular-file match.
pub(crate) fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
