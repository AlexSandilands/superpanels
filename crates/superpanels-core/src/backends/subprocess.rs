//! Shared subprocess helper for non-D-Bus backends (`SPEC.md` §10.3).

use std::ffi::OsStr;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::BackendError;

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
#[allow(dead_code)] // reason: stderr currently unread; preserved for future diagnostics
pub(crate) struct CapturedOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// Spawn with `LC_ALL=C`, wait up to `timeout`, capture stdout + stderr.
pub(crate) fn run(
    program: &str,
    args: &[&OsStr],
    timeout: Duration,
) -> Result<CapturedOutput, BackendError> {
    let cmd_display = display_cmdline(program, args);
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BackendError::Subprocess {
            cmd: cmd_display.clone(),
            exit: -1,
            stderr: e.to_string(),
        })?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| BackendError::Subprocess {
                        cmd: cmd_display.clone(),
                        exit: -1,
                        stderr: e.to_string(),
                    })?;
                let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                if !status.success() {
                    return Err(BackendError::Subprocess {
                        cmd: cmd_display,
                        exit: status.code().unwrap_or(-1),
                        stderr,
                    });
                }
                return Ok(CapturedOutput { stdout, stderr });
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    return Err(BackendError::Timeout {
                        cmd: cmd_display,
                        seconds: timeout.as_secs(),
                    });
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                return Err(BackendError::Subprocess {
                    cmd: cmd_display,
                    exit: -1,
                    stderr: e.to_string(),
                });
            }
        }
    }
}

pub(crate) fn which(bin: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path) {
        if dir.join(bin).is_file() {
            return true;
        }
    }
    false
}

fn display_cmdline(program: &str, args: &[&OsStr]) -> String {
    let mut s = String::from(program);
    for arg in args {
        s.push(' ');
        s.push_str(&arg.to_string_lossy());
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used)] // reason: tests fail loudly on subprocess setup errors
mod tests {
    use super::*;

    #[test]
    fn run_true_succeeds_with_empty_output() {
        if !which("true") {
            return;
        }
        let out = run("true", &[], DEFAULT_TIMEOUT).expect("true should succeed");
        assert!(out.stdout.is_empty());
    }

    #[test]
    fn run_false_returns_subprocess_error() {
        if !which("false") {
            return;
        }
        let err = run("false", &[], DEFAULT_TIMEOUT).expect_err("false should fail");
        assert!(
            matches!(err, BackendError::Subprocess { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn missing_binary_returns_subprocess_error() {
        let err = run(
            "definitely-not-a-real-binary-xyz-superpanels",
            &[],
            DEFAULT_TIMEOUT,
        )
        .expect_err("missing binary should fail");
        assert!(matches!(err, BackendError::Subprocess { .. }));
    }

    #[test]
    fn which_finds_a_known_unix_tool_or_returns_false() {
        assert!(!which("definitely-not-a-real-binary-xyz-superpanels"));
    }

    #[test]
    fn display_cmdline_joins_program_and_args_with_spaces() {
        let args: [&OsStr; 2] = [OsStr::new("--flag"), OsStr::new("value")];
        let s = display_cmdline("prog", &args);
        assert_eq!(s, "prog --flag value");
    }
}
