//! Autostart toggle `#[tauri::command]`s. Wraps `crate::autostart` with the
//! IPC return-shape the frontend expects. Both touch the filesystem, so they
//! run off the main thread like every other command.

use serde_json::{Value, json};

use crate::errors::IpcError;

#[tauri::command]
pub(crate) async fn set_autostart(enabled: bool) -> Result<Value, IpcError> {
    super::run_off_main(move || {
        crate::autostart::set_enabled(enabled).map(|()| json!({ "enabled": enabled }))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_autostart() -> Value {
    tauri::async_runtime::spawn_blocking(|| json!({ "enabled": crate::autostart::is_enabled() }))
        .await
        .unwrap_or_else(|_| json!({ "enabled": false }))
}
