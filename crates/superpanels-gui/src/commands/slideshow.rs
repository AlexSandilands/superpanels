//! Slideshow control `#[tauri::command]`s.

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn slideshow_next(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main("slideshow_next", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn slideshow_prev(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main("slideshow_prev", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn slideshow_pause(
    paused: Option<bool>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let mut params = json!({});
    if let Some(p) = paused {
        params = json!({ "paused": p });
    }
    bridge::call_off_main("slideshow_pause", params, state.config_path()).await
}
