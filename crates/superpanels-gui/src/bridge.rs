//! Daemon-first / in-process bridge.
//!
//! Each Tauri command calls [`call`] with an IPC method and JSON params. We
//! try the daemon socket first; if no daemon is running we synthesise the
//! same JSON payload from a direct call into `superpanels-core`. The Svelte
//! frontend never sees the difference.

use std::path::Path;

use serde_json::{Value, json};
use superpanels_core::ipc::client as ipc_client;
use superpanels_core::ipc::{IpcResponse, socket_path};

use crate::errors::IpcError;

/// Result of a successful daemon (or synthesised) IPC call.
pub(crate) type CallResult = Result<Value, IpcError>;

/// Whether a given method should be attempted in-process when no daemon is up.
/// Slideshow control commands genuinely need daemon state and have no useful
/// in-process fallback.
fn has_in_process_fallback(method: &str) -> bool {
    !matches!(
        method,
        "slideshow_next" | "slideshow_prev" | "slideshow_pause"
    )
}

pub(crate) fn call(method: &str, params: Value, config_path: Option<&Path>) -> CallResult {
    if let Some(mut stream) = ipc_client::try_connect(&socket_path()) {
        // Mid-call socket / framing failure is a transport problem, not a
        // logical rejection from the daemon — keep the two paths distinct so
        // callers can `match err { IpcError::DaemonUnreachable(_) => … }`
        // without inspecting strings (confused-deputy guard, `SPEC §17`).
        let resp = ipc_client::call(&mut stream, method, params)
            .map_err(|e| IpcError::DaemonUnreachable(e.to_string()))?;
        return resolve(resp);
    }
    if !has_in_process_fallback(method) {
        return Err(IpcError::DaemonUnreachable(format!(
            "no daemon is running — start one with `superpanels-daemon`; \
             '{method}' has no in-process fallback"
        )));
    }
    crate::commands::in_process::dispatch(method, &params, config_path)
}

fn resolve(resp: IpcResponse) -> CallResult {
    if resp.is_ok() {
        Ok(resp.result.unwrap_or(Value::Null))
    } else {
        // Logical rejection from the daemon (e.g. "path outside roots").
        // Stays as `Daemon` so the `library_thumbnail` fallback only fires
        // on `DaemonUnreachable`.
        Err(IpcError::Daemon(
            resp.error.unwrap_or_else(|| "daemon error".to_owned()),
        ))
    }
}

/// Helper: synthesise an `IpcResponse`-shaped success body so an in-process
/// dispatch returns the same JSON the daemon would have. Kept here so call
/// sites don't reach into `IpcResponse` themselves.
pub(crate) fn ok_payload<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

pub(crate) fn ok_unit() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slideshow_methods_have_no_in_process_fallback() {
        assert!(!has_in_process_fallback("slideshow_next"));
        assert!(!has_in_process_fallback("slideshow_prev"));
        assert!(!has_in_process_fallback("slideshow_pause"));
    }

    #[test]
    fn other_methods_have_in_process_fallback() {
        assert!(has_in_process_fallback("detect_monitors"));
        assert!(has_in_process_fallback("apply_profile"));
        assert!(has_in_process_fallback("get_config"));
    }

    #[test]
    fn ok_payload_serialises_value() {
        let v: Value = ok_payload(vec![1u32, 2, 3]);
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }
}
