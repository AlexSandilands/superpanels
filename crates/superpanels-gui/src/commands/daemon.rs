//! Daemon-lifecycle `#[tauri::command]`s.
//!
//! `daemon_status` is a cheap socket probe the GUI polls to drive its
//! "daemon not running" banner. `start_daemon` spawns the bundled
//! `superpanels-daemon` binary; the daemon self-daemonises (re-execs
//! `--foreground` in the background) so the GUI just needs to fire it
//! once and drop the child handle.

#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};
use superpanels_core::ipc::{client as ipc_client, socket_path};

use crate::errors::IpcError;

const DAEMON_BIN: &str = "superpanels-daemon";

#[tauri::command]
pub(crate) fn daemon_status() -> Value {
    let connected = ipc_client::try_connect(&socket_path()).is_some();
    json!({ "connected": connected })
}

#[tauri::command]
pub(crate) fn start_daemon() -> Result<Value, IpcError> {
    let exe = locate_daemon_exe();
    Command::new(&exe).spawn().map_err(|e| {
        IpcError::internal(format!(
            "could not spawn '{}': {e} — is `superpanels-daemon` on $PATH?",
            exe.display()
        ))
    })?;
    Ok(json!({ "exe": exe.display().to_string() }))
}

/// Resolve the daemon binary path.
///
/// Prefer a sibling of the current GUI executable (the dev `cargo run`
/// layout puts both binaries in `target/debug/`, and packaged installs
/// typically share `/usr/bin`). Fall back to the bare name so $PATH
/// lookup applies if the sibling is missing.
fn locate_daemon_exe() -> PathBuf {
    if let Ok(self_exe) = std::env::current_exe() {
        if let Some(dir) = self_exe.parent() {
            let neighbour = dir.join(DAEMON_BIN);
            if neighbour.exists() {
                return neighbour;
            }
        }
    }
    PathBuf::from(DAEMON_BIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_status_returns_connected_field() {
        let v = daemon_status();
        assert!(
            v.get("connected").is_some_and(Value::is_boolean),
            "expected boolean `connected` field, got {v}"
        );
    }

    #[test]
    fn locate_daemon_exe_returns_bare_name_when_sibling_absent() {
        // We can't easily mock `current_exe()`, but the fallback path is
        // exercised on most dev setups where the cargo target dir doesn't
        // contain a `superpanels-daemon` next to the test binary.
        let p = locate_daemon_exe();
        let last = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        assert_eq!(
            last,
            DAEMON_BIN,
            "unexpected resolved path: {}",
            p.display()
        );
    }
}
