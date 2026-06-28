//! Config get/save `#[tauri::command]`s.

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};
use superpanels_core::Config;
use tauri_plugin_opener::OpenerExt;

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

/// Open the active `config.toml` in the user's default handler. The file is
/// materialised with defaults first when a fresh install hasn't saved one yet,
/// so there's always something to edit. Hand-off to the portal-aware opener is
/// fire-and-forget — no timeout, since some handlers block for the editor's
/// lifetime.
#[tauri::command]
pub(crate) async fn open_config_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let override_path = state.config_path();
    let resolved = crate::commands::run_off_main(move || {
        let path = match override_path {
            Some(p) => p,
            None => Config::default_path()?,
        };
        if !path.exists() {
            Config::load_or_default_from(&path)?.save_to(&path)?;
        }
        Ok(json!({ "path": path.to_string_lossy() }))
    })
    .await?;

    let path = resolved
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| IpcError::internal("config path unavailable"))?;

    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| IpcError::internal(format!("could not open config file: {e}")))?;
    tracing::info!(path, "opened config file");
    Ok(json!({}))
}
