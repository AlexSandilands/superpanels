//! Autostart toggle `#[tauri::command]`s. Wraps `crate::autostart` with the
//! IPC return-shape the frontend expects.

use serde_json::{Value, json};

use crate::errors::IpcError;

#[tauri::command]
pub(crate) fn set_autostart(enabled: bool) -> Result<Value, IpcError> {
    crate::autostart::set_enabled(enabled).map(|()| json!({ "enabled": enabled }))
}

#[tauri::command]
pub(crate) fn get_autostart() -> Value {
    json!({ "enabled": crate::autostart::is_enabled() })
}
