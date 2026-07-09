//! Window-chrome `#[tauri::command]`s.

use tauri::State;

use crate::window_chrome::{DragRegions, Rect};

/// Publish the window-relative rectangles that drag the window when pressed.
/// Unlike every other command this one is synchronous: it takes a lock and
/// swaps a `Vec`, and the GTK press handler reads the result on the very next
/// button press, so a trip through the blocking pool would only add latency.
// reason: `tauri::State` is always passed by value into a command handler.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub(crate) fn set_drag_regions(regions: Vec<Rect>, state: State<'_, DragRegions>) {
    state.set(regions);
}
