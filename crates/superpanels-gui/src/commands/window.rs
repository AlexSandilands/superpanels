//! Window-chrome `#[tauri::command]`s.

use serde_json::{Value, json};
use tauri::State;

use crate::window_chrome::{DragRegions, Rect, bands, clamp_to_i32};

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
pub(crate) fn resize_bands(window: tauri::Window) -> Value {
    let scale = window.scale_factor().map_or(1, |s| clamp_to_i32(s).max(1));
    let (edge, corner) = bands(scale);
    json!({ "edge": edge, "corner": corner })
}
