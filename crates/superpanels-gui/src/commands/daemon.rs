//! Daemon-lifecycle `#[tauri::command]`s.
//!
//! `daemon_status` is a cheap socket probe the GUI polls to drive its
//! "daemon not running" banner. `start_daemon` spawns the bundled
//! `superpanels-daemon` binary; the daemon self-daemonises (re-execs
//! `--foreground` in the background) so the GUI only fires it once and
//! reaps the short-lived intermediate child.

#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};
use superpanels_core::ipc::{client as ipc_client, socket_path};

use crate::errors::IpcError;

const DAEMON_BIN: &str = "superpanels-daemon";

#[tauri::command]
pub(crate) async fn daemon_status() -> Value {
    // `try_connect` is a blocking connect; the GUI polls this, and a stale
    // socket can stall it — keep it off the main thread.
    tauri::async_runtime::spawn_blocking(|| {
        let connected = ipc_client::try_connect(&socket_path()).is_some();
        json!({ "connected": connected })
    })
    .await
    .unwrap_or_else(|_| json!({ "connected": false }))
}

#[tauri::command]
pub(crate) async fn start_daemon() -> Result<Value, IpcError> {
    super::run_off_main(|| {
        let spawned = ensure_daemon_running()?;
        Ok(json!({ "exe": spawned.map(|p| p.display().to_string()) }))
    })
    .await
}

/// Spawn the daemon only if none is currently listening.
///
/// Idempotent and safe to call unconditionally: the daemon binds its socket
/// exclusively (`superpanels-daemon` refuses to start when a live one exists),
/// so a redundant spawn is a harmless no-op. Returns the resolved exe path when
/// a spawn actually happened, or `None` when a daemon was already up.
///
/// Blocking — call from a worker thread, not the webview's main thread.
pub(crate) fn ensure_daemon_running() -> Result<Option<PathBuf>, IpcError> {
    if ipc_client::try_connect(&socket_path()).is_some() {
        return Ok(None);
    }
    let exe = locate_daemon_exe();
    let mut child = Command::new(&exe).spawn().map_err(|e| {
        IpcError::internal(format!(
            "could not spawn '{}': {e} — is `superpanels-daemon` on $PATH?",
            exe.display()
        ))
    })?;
    // The child re-execs `--foreground` detached and exits within
    // milliseconds; reap it off-thread so it doesn't linger as a zombie for
    // the GUI's lifetime (#82).
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(Some(exe))
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
        let v = tauri::async_runtime::block_on(daemon_status());
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
