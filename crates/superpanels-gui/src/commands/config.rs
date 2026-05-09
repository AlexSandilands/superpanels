//! Config get/save `#[tauri::command]`s (`SPEC.md` §12.4 / §14.1).

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[tauri::command]
pub(crate) fn get_config(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("get_config", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub(crate) fn save_config(
    config: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "save_config",
        json!({ "config": config }),
        state.config_path().as_deref(),
    )
}
