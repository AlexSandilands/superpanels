//! Config get/save `#[tauri::command]`s.

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn get_config(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call_off_main("get_config", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn save_config(
    config: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "save_config",
        json!({ "config": config }),
        state.config_path(),
    )
    .await
}
