//! Profile-management `#[tauri::command]`s.

// reason: Tauri requires owned types in `#[tauri::command]` signatures (see
// `commands.rs` for the full rationale).
#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn list_profiles(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main("list_profiles", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn apply_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    if name.trim().is_empty() {
        return Err(IpcError::invalid("profile name is empty"));
    }
    bridge::call_off_main(
        "apply_profile",
        json!({ "name": name }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn save_profile(
    profile: Value,
    recompute_topology: Option<bool>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "save_profile",
        json!({
            "profile": profile,
            "recompute_topology": recompute_topology.unwrap_or(false),
        }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn apply_canvas(
    profile: Value,
    active_name: Option<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "apply_canvas",
        json!({ "profile": profile, "active_name": active_name }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn delete_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "delete_profile",
        json!({ "name": name }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn duplicate_profile(
    name: String,
    new_name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "duplicate_profile",
        json!({ "name": name, "new_name": new_name }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn rename_profile(
    name: String,
    new_name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "rename_profile",
        json!({ "name": name, "new_name": new_name }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn update_profile_monitor_state(
    profile: String,
    stable_id: String,
    placement: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "update_profile_monitor_state",
        json!({ "profile": profile, "stable_id": stable_id, "placement": placement }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn update_profile_image_transform(
    profile: String,
    image_rect_mm: Option<Value>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "update_profile_image_transform",
        json!({
            "profile": profile,
            "image_rect_mm": image_rect_mm,
        }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn update_profile_source(
    profile: String,
    source: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "update_profile_source",
        json!({ "profile": profile, "source": source }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn list_schedules(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main("list_schedules", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn save_schedules(
    schedules: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "save_schedules",
        json!({ "schedules": schedules }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn set_schedules_paused(
    paused: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "set_schedules_paused",
        json!({ "paused": paused }),
        state.config_path(),
    )
    .await
}
