//! Background re-exec: detach the daemon from the launching terminal session.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::Cli;

/// Re-exec the daemon in the foreground as a detached child so it survives the
/// launching terminal's SIGHUP.
///
/// Packaged installs should prefer the systemd user unit (`--install-unit`),
/// which runs `--foreground` directly and never reaches this path.
pub(crate) fn daemonize(cli: &Cli) -> Result<()> {
    let exe = std::env::current_exe().context("resolving current executable")?;
    let setsid = find_in_path("setsid");
    if setsid.is_none() {
        warn!(
            "setsid(1) not found; detaching via a new process group only. The \
             daemon may still be killed by SIGHUP if the launching terminal \
             closes — install util-linux, or run under the systemd user unit \
             (superpanels-daemon --install-unit)."
        );
    }
    let mut cmd = build_daemon_command(&exe, cli, setsid.as_deref());
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning background daemon process")?;
    info!(socket = ?cli.socket, detached = setsid.is_some(), "daemon started in background");
    Ok(())
}

/// Build the re-exec command for the background daemon.
///
/// Prefers `setsid(1)`: it starts the daemon in a fresh session with no
/// controlling terminal, so a terminal hangup can never reach it. Calling
/// `setsid(2)` in-process would require `pre_exec` (unsafe) or a manual fork,
/// both barred by `#![forbid(unsafe_code)]`, hence the subprocess. When
/// `setsid(1)` is absent, falls back to `process_group(0)` so the daemon at
/// least leaves the launching foreground process group (weaker: it keeps the
/// controlling terminal, so SIGHUP is still possible).
fn build_daemon_command(exe: &Path, cli: &Cli, setsid: Option<&Path>) -> std::process::Command {
    let mut cmd = if let Some(setsid) = setsid {
        let mut cmd = std::process::Command::new(setsid);
        cmd.arg(exe);
        cmd
    } else {
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(exe);
        cmd.process_group(0);
        cmd
    };
    cmd.arg("--foreground");
    if let Some(sock) = &cli.socket {
        cmd.arg("--socket").arg(sock);
    }
    if let Some(cfg) = &cli.config {
        cmd.arg("--config").arg(cfg);
    }
    for _ in 0..cli.verbose {
        cmd.arg("-v");
    }
    if cli.quiet {
        cmd.arg("--quiet");
    }
    cmd
}

/// First executable match for `bin` on `$PATH`, or `None` if not found.
fn find_in_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| is_executable_file(candidate))
}

/// A non-executable `setsid` on `$PATH` must not be selected — spawning it
/// would fail with no fallback to the `process_group(0)` path.
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use std::ffi::{OsStr, OsString};

    use super::*;

    fn cli_with(socket: Option<PathBuf>, config: Option<PathBuf>, verbose: u8, quiet: bool) -> Cli {
        Cli {
            foreground: false,
            socket,
            config,
            install_unit: false,
            verbose,
            quiet,
        }
    }

    #[test]
    fn build_command_prefers_setsid_and_passes_exe_as_first_arg() {
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let setsid = PathBuf::from("/usr/bin/setsid");
        let cmd = build_daemon_command(&exe, &cli_with(None, None, 0, false), Some(&setsid));
        assert_eq!(cmd.get_program(), setsid.as_os_str());
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args, vec![exe.as_os_str(), OsStr::new("--foreground")]);
    }

    #[test]
    fn build_command_without_setsid_execs_daemon_directly() {
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let cmd = build_daemon_command(&exe, &cli_with(None, None, 0, false), None);
        assert_eq!(cmd.get_program(), exe.as_os_str());
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args, vec![OsStr::new("--foreground")]);
    }

    #[test]
    fn build_command_forwards_socket_config_and_verbosity() {
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let sock = PathBuf::from("/run/user/1000/sp.sock");
        let cfg = PathBuf::from("/home/me/config.toml");
        let cli = cli_with(Some(sock.clone()), Some(cfg.clone()), 2, true);
        let cmd = build_daemon_command(&exe, &cli, None);
        let args: Vec<OsString> = cmd.get_args().map(OsStr::to_owned).collect();
        assert_eq!(
            args,
            vec![
                OsString::from("--foreground"),
                OsString::from("--socket"),
                sock.into_os_string(),
                OsString::from("--config"),
                cfg.into_os_string(),
                OsString::from("-v"),
                OsString::from("-v"),
                OsString::from("--quiet"),
            ]
        );
    }

    #[test]
    fn find_in_path_returns_none_for_missing_binary() {
        assert!(find_in_path("definitely-not-a-real-binary-xyz-superpanels").is_none());
    }

    #[test]
    fn non_executable_file_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("setsid");
        std::fs::write(&file, b"not a binary").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_executable_file(&file));
    }

    #[test]
    fn executable_file_is_accepted() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("setsid");
        std::fs::write(&file, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable_file(&file));
    }

    #[test]
    fn directory_is_rejected_even_with_exec_bit() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_executable_file(dir.path()));
    }
}
