//! Monitor detection / configuration `#[tauri::command]`s (`SPEC.md` §6 / §12.4).

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};
use superpanels_core::ipc::validate as v;

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[tauri::command]
pub(crate) fn detect_monitors(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("redetect", json!({}), state.config_path().as_deref())?;
    bridge::call("detect_monitors", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub(crate) fn redetect(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("redetect", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub(crate) fn set_monitor_physical_size(
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
    bridge::call(
        "set_monitor_physical_size",
        params,
        state.config_path().as_deref(),
    )
}
