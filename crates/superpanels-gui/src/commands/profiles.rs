//! Profile-management `#[tauri::command]`s (`SPEC.md` §12.4).

// reason: Tauri requires owned types in `#[tauri::command]` signatures (see
// `commands.rs` for the full rationale).
#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[tauri::command]
pub(crate) fn list_profiles(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("list_profiles", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub(crate) fn apply_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    if name.trim().is_empty() {
        return Err(IpcError::invalid("profile name is empty"));
    }
    bridge::call(
        "apply_profile",
        json!({ "name": name }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub(crate) fn save_profile(
    profile: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "save_profile",
        json!({ "profile": profile }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub(crate) fn delete_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "delete_profile",
        json!({ "name": name }),
        state.config_path().as_deref(),
    )
}
