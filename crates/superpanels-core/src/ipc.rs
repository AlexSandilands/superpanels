//! IPC message types and socket-path helper (`SPEC.md` §5.3).
//!
//! Wire format: 4-byte big-endian length followed by a UTF-8 JSON body.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod client;
pub mod validate;

pub const PROTOCOL_VERSION: u32 = 1;

/// Outbound request sent by a client over the Unix socket.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    pub v: u32,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Inbound response sent back by the daemon. Exactly one of `result` / `error`
/// is `Some` in a well-formed response.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub v: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IpcResponse {
    /// Successful response; `payload` is JSON-encoded into `result`.
    pub fn success(payload: impl Serialize) -> Self {
        let val = serde_json::to_value(payload).unwrap_or(serde_json::Value::Null);
        Self {
            v: PROTOCOL_VERSION,
            result: Some(val),
            error: None,
        }
    }

    pub fn failure(msg: impl std::fmt::Display) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            result: None,
            error: Some(msg.to_string()),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Snapshot of daemon runtime state returned by the `current_state` IPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub version: u32,
    pub active_profile: Option<String>,
    pub slideshow: Option<SlideshowSummary>,
    /// Unix seconds of the most recent successful apply; `None` if never applied.
    pub last_apply_unix_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideshowSummary {
    pub current_index: Option<usize>,
    pub history_len: usize,
    pub paused: bool,
}

/// Unix domain socket path for the daemon IPC.
///
/// Tries `$XDG_RUNTIME_DIR/superpanels/daemon.sock`; falls back to
/// `/tmp/superpanels-<uid>/daemon.sock` (0700 dir created by the daemon).
pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(&dir);
        if !p.as_os_str().is_empty() {
            return p.join("superpanels").join("daemon.sock");
        }
    }
    let uid = uid_from_proc().unwrap_or(0);
    std::env::temp_dir()
        .join(format!("superpanels-{uid}"))
        .join("daemon.sock")
}

fn uid_from_proc() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:\t") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests — failure is a bug, not a runtime error
mod tests {
    use super::*;

    #[test]
    fn success_response_has_no_error_field() {
        let r = IpcResponse::success(serde_json::json!({"ok": true}));
        assert!(r.is_ok());
        assert!(r.error.is_none());
        assert!(r.result.is_some());
    }

    #[test]
    fn failure_response_has_no_result_field() {
        let r = IpcResponse::failure("something went wrong");
        assert!(!r.is_ok());
        assert!(r.result.is_none());
        assert_eq!(r.error.as_deref(), Some("something went wrong"));
    }

    #[test]
    fn round_trips_through_json() {
        let orig = IpcResponse::success(42u32);
        let json = serde_json::to_string(&orig).unwrap();
        let back: IpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.result, orig.result);
    }
}
