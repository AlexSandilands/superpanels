//! Monitor detection / configuration `#[tauri::command]`s.

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use superpanels_core::ipc::client as ipc_client;
use superpanels_core::ipc::socket_path;
use superpanels_core::ipc::validate as v;
use tauri::{AppHandle, Emitter, Manager};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

/// Tauri event the frontend listens for (`ui/src/lib/events/window.ts`).
const MONITOR_CHANGED_EVENT: &str = "monitors://changed";

/// Backoff between reconnect attempts when the daemon socket is unreachable.
const RECONNECT_BACKOFF: Duration = Duration::from_secs(3);

#[tauri::command]
pub(crate) async fn detect_monitors(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main("redetect", json!({}), state.config_path()).await?;
    bridge::call_off_main("detect_monitors", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn redetect(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call_off_main("redetect", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn set_monitor_physical_size(
    stable_id: Option<String>,
    name: Option<String>,
    physical_mm: [f64; 2],
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    if stable_id.as_deref().is_none_or(str::is_empty) && name.as_deref().is_none_or(str::is_empty) {
        return Err(IpcError::invalid("stable_id or name is required"));
    }
    if let Some(id) = stable_id.as_deref().filter(|s| !s.is_empty()) {
        v::validate_monitor_id_string(id, "stable_id").map_err(|e| IpcError::invalid(e.0))?;
    }
    if let Some(n) = name.as_deref().filter(|s| !s.is_empty()) {
        v::validate_monitor_id_string(n, "name").map_err(|e| IpcError::invalid(e.0))?;
    }
    v::validate_physical_mm(physical_mm).map_err(|e| IpcError::invalid(e.0))?;
    let mut params = json!({ "physical_mm": physical_mm });
    if let Some(id) = stable_id {
        if !id.is_empty() {
            params["stable_id"] = Value::String(id);
        }
    }
    if let Some(n) = name {
        if !n.is_empty() {
            params["name"] = Value::String(n);
        }
    }
    bridge::call_off_main("set_monitor_physical_size", params, state.config_path()).await
}

/// Spawn a dedicated OS thread that runs the daemon's
/// `wait_for_monitor_change` long-poll in a loop and emits
/// [`MONITOR_CHANGED_EVENT`] on every change tick. Sync IPC stays out of
/// Tauri's async runtime; the thread exits cleanly on shutdown.
///
/// On any transport failure (daemon down, mid-call I/O), we back off and
/// retry — when the daemon comes back the long-poll resumes transparently.
pub(crate) fn spawn_push_relay(handle: AppHandle) {
    if let Err(e) = std::thread::Builder::new()
        .name("monitor-push-relay".into())
        .spawn(move || run_push_relay(&handle))
    {
        tracing::warn!(error = %e, "could not spawn monitor push relay");
    }
}

fn run_push_relay(app: &AppHandle) {
    let shutdown = || {
        app.try_state::<Arc<AppState>>()
            .is_some_and(|s| s.shutting_down())
    };
    while !shutdown() {
        match poll_once(app) {
            Ok(()) => {}
            Err(PollError::DaemonUnreachable) => {
                std::thread::sleep(RECONNECT_BACKOFF);
            }
            Err(PollError::Transport(e)) => {
                tracing::debug!(error = %e, "monitor push relay transport error");
                std::thread::sleep(RECONNECT_BACKOFF);
            }
            Err(PollError::Daemon(msg)) => {
                tracing::warn!(error = %msg, "daemon rejected wait_for_monitor_change");
                std::thread::sleep(RECONNECT_BACKOFF);
            }
        }
    }
}

#[derive(Debug)]
enum PollError {
    DaemonUnreachable,
    Transport(ipc_client::ClientError),
    Daemon(String),
}

impl From<ipc_client::ClientError> for PollError {
    fn from(e: ipc_client::ClientError) -> Self {
        Self::Transport(e)
    }
}

fn poll_once(app: &AppHandle) -> Result<(), PollError> {
    let mut stream = ipc_client::try_connect(&socket_path()).ok_or(PollError::DaemonUnreachable)?;
    let resp = ipc_client::call(&mut stream, "wait_for_monitor_change", json!({}))?;
    if !resp.is_ok() {
        return Err(PollError::Daemon(
            resp.error.unwrap_or_else(|| "unknown".to_owned()),
        ));
    }
    let changed = resp
        .result
        .as_ref()
        .and_then(|v| v.get("changed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if changed {
        if let Err(e) = app.emit(MONITOR_CHANGED_EVENT, ()) {
            tracing::warn!(error = %e, event = MONITOR_CHANGED_EVENT, "emit failed");
        } else {
            tracing::debug!("relayed monitors://changed from daemon push");
        }
    }
    Ok(())
}
