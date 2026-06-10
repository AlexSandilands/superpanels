//! Tray-icon style `#[tauri::command]`s. Persist the user's choice and swap
//! the live tray glyph. Both touch the filesystem, so they run off the main
//! thread like every other command.

use serde_json::{Value, json};
use tauri::AppHandle;

use crate::errors::IpcError;
use crate::tray::icon::TrayIconStyle;

#[tauri::command]
pub(crate) async fn set_tray_icon_style(app: AppHandle, style: String) -> Result<Value, IpcError> {
    let parsed = TrayIconStyle::parse(&style)
        .ok_or_else(|| IpcError::invalid(format!("unknown tray icon style: {style}")))?;
    super::run_off_main(move || {
        crate::tray::apply_icon_style(&app, parsed)?;
        Ok(json!({ "style": parsed.as_str() }))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_tray_icon_style() -> Value {
    tauri::async_runtime::spawn_blocking(
        || json!({ "style": crate::tray::icon::load_style().as_str() }),
    )
    .await
    .unwrap_or_else(|_| json!({ "style": TrayIconStyle::default().as_str() }))
}
