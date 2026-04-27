//! Shared subprocess helper for display detectors (`SPEC.md` §6).

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::DetectError;

/// Spawn with `LC_ALL=C` and `NO_COLOR=1`, wait up to `timeout`, return stdout.
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
