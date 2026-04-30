//! `#[tauri::command]` wrappers for the IPC surface in `SPEC.md` §12.4.
//!
//! Each command is a 3-line bridge: validate args → call [`crate::bridge`] →
//! return `Result<T, IpcError>`. The bridge picks daemon-or-in-process, so
//! these wrappers stay shape-only.

// reason: Tauri requires owned `String`, `Value`, and `tauri::State` types
// in `#[tauri::command]` signatures (the macro deserialises them from JSON).
// `needless_pass_by_value` fires on every command and the suggested `&_`
// signatures don't compile with the macro.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::bridge;
use crate::errors::IpcError;
use crate::state::{AppState, RuntimeSnapshot};

pub mod in_process;

// --- shared payload types (TS-exported) --------------------------------------

/// Filter forwarded to `library_list`. Mirrors `superpanels_core::LibraryFilter`
/// but is defined in the GUI crate so `ts-rs` can export it for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct LibraryFilter {
    #[ts(optional)]
    pub tag: Option<String>,
    #[ts(optional)]
    pub min_width: Option<u32>,
    #[ts(optional)]
    pub min_height: Option<u32>,
    #[ts(optional)]
    pub aspect_min: Option<f32>,
    #[ts(optional)]
    pub aspect_max: Option<f32>,
    #[ts(optional)]
    pub offset: Option<u32>,
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct PreviewArgs {
    pub image: String,
    pub offset_px: [i32; 2],
    pub bezel_h_mm: f32,
    pub bezel_v_mm: f32,
    pub fit: String,
}

// --- command bindings --------------------------------------------------------

#[tauri::command]
pub fn detect_monitors(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("redetect", json!({}), state.config_path().as_deref())?;
    bridge::call("detect_monitors", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn list_profiles(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("list_profiles", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn apply_profile(
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
pub fn save_profile(
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
pub fn delete_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "delete_profile",
        json!({ "name": name }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn preview_crop(
    args: PreviewArgs,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "preview_crop",
        serde_json::to_value(&args).unwrap_or(Value::Null),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn library_list(
    filter: LibraryFilter,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "library_list",
        serde_json::to_value(&filter).unwrap_or(Value::Null),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn library_thumbnail(
    path: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(IpcError::invalid("thumbnail path must be absolute"));
    }
    bridge::call(
        "library_thumbnail",
        json!({ "path": p.to_string_lossy() }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn library_tag(
    path: String,
    tag: String,
    on: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "library_tag",
        json!({ "path": path, "tag": tag, "on": on }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn slideshow_next(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("slideshow_next", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn slideshow_prev(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("slideshow_prev", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn slideshow_pause(
    paused: Option<bool>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let mut params = json!({});
    if let Some(p) = paused {
        params = json!({ "paused": p });
    }
    bridge::call("slideshow_pause", params, state.config_path().as_deref())
}

#[tauri::command]
pub fn get_config(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("get_config", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn save_config(
    config: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "save_config",
        json!({ "config": config }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn redetect(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("redetect", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn current_state(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    let v = bridge::call("current_state", json!({}), state.config_path().as_deref())?;
    state.set_snapshot(parse_runtime_snapshot(&v));
    Ok(v)
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<Value, IpcError> {
    crate::autostart::set_enabled(enabled).map(|()| json!({ "enabled": enabled }))
}

#[tauri::command]
pub fn get_autostart() -> Result<Value, IpcError> {
    Ok(json!({ "enabled": crate::autostart::is_enabled() }))
}

fn parse_runtime_snapshot(v: &Value) -> RuntimeSnapshot {
    let active_profile = v
        .get("active_profile")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let paused = v
        .get("slideshow")
        .and_then(|s| s.get("paused"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let current_filename = v
        .get("current_filename")
        .and_then(Value::as_str)
        .map(str::to_owned);
    RuntimeSnapshot {
        active_profile,
        current_filename,
        paused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_snapshot_handles_minimal_payload() {
        let v = json!({"active_profile": "home", "slideshow": {"paused": true}});
        let snap = parse_runtime_snapshot(&v);
        assert_eq!(snap.active_profile.as_deref(), Some("home"));
        assert!(snap.paused);
    }

    #[test]
    fn parse_runtime_snapshot_defaults_when_fields_missing() {
        let v = json!({});
        let snap = parse_runtime_snapshot(&v);
        assert!(snap.active_profile.is_none());
        assert!(!snap.paused);
    }
}
