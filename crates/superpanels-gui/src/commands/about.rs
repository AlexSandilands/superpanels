//! `#[tauri::command]`s backing the Settings → About panel.

// reason: `AppHandle` is cheap-to-clone (Arc inside) but Tauri hands commands
// an owned handle; taking it by value is the framework convention.
#![allow(clippy::needless_pass_by_value)]

use serde_json::{Value, json};
use tauri_plugin_opener::OpenerExt;

use crate::errors::IpcError;

/// Open this build's GitHub release page in the user's browser. The URL is
/// built entirely from compile-time crate metadata — never from the (hostile)
/// webview — so the renderer can only ever open our own release page for the
/// version it was built as. A dev build (version `0.0.0`, which has no tag)
/// falls back to the releases index.
#[tauri::command]
pub(crate) fn open_release_page(app: tauri::AppHandle) -> Result<Value, IpcError> {
    let repo = env!("CARGO_PKG_REPOSITORY");
    let version = env!("CARGO_PKG_VERSION");

    let url = if version == "0.0.0" {
        format!("{repo}/releases")
    } else {
        format!("{repo}/releases/tag/v{version}")
    };

    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| IpcError::internal(format!("could not open release page: {e}")))?;
    tracing::info!(url, "opened release page");
    Ok(json!({}))
}
