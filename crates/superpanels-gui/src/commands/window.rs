//! Window-chrome `#[tauri::command]`s.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::state::AppState;
use crate::window_chrome::{DragRegions, Rect, bands, window_scale};

/// Widths of the window's resize grab regions, in logical pixels. Exported so
/// the frontend's cursor geometry can't drift from the backend's hit test.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub(crate) struct ResizeBands {
    pub(crate) edge: i32,
    pub(crate) corner: i32,
}

/// Publish the window-relative rectangles that drag the window when pressed.
/// Unlike every other command this one is synchronous: it takes a lock and
/// swaps a `Vec`, and the GTK press handler reads the result on the very next
/// button press, so a trip through the blocking pool would only add latency.
// reason: `tauri::State` is always passed by value into a command handler.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub(crate) fn set_drag_regions(regions: Vec<Rect>, state: State<'_, DragRegions>) {
    tracing::debug!(count = regions.len(), "drag regions updated");
    state.set(regions);
}

/// Edge and corner grab bands for the window's current scale. The frontend
/// paints its resize cursors over exactly these rather than re-deriving the
/// numbers and drifting from the hit test that actually starts the drag.
// reason: `tauri::Window` is always passed by value into a command handler.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub(crate) fn resize_bands(window: tauri::Window) -> ResizeBands {
    let (edge, corner) = bands(window_scale(&window));
    ResizeBands { edge, corner }
}

/// Boot handshake for a window rebuilt from tray "Settings…": returns (and
/// clears) whether the panel should open. A freshly loaded page has no
/// `tray://open-settings` listener yet, so the tray stashes the intent in
/// [`AppState`] and the frontend drains it here once listeners are up.
// reason: `tauri::State` is always passed by value into a command handler.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub(crate) fn take_pending_open_settings(state: State<'_, Arc<AppState>>) -> bool {
    state.take_pending_open_settings()
}
