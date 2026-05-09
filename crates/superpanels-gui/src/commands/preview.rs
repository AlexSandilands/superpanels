//! `preview_crop` `#[tauri::command]` and the `PreviewArgs` payload type
//! (`SPEC.md` §12.4).

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub(crate) struct PreviewArgs {
    pub(crate) image: String,
    pub(crate) offset_px: [i32; 2],
    pub(crate) bezel_h_mm: f32,
    pub(crate) bezel_v_mm: f32,
    pub(crate) fit: String,
}

#[tauri::command]
pub(crate) fn preview_crop(
    args: PreviewArgs,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let params = serde_json::to_value(&args)
        .map_err(|e| IpcError::internal(format!("PreviewArgs serialise: {e}")))?;
    bridge::call("preview_crop", params, state.config_path().as_deref())
}
