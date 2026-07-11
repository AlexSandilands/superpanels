//! `#[tauri::command]` wrappers for the IPC surface.
//!
//! Each command is a 3-line bridge: validate args → call [`crate::bridge`] →
//! return `Result<T, IpcError>`. The bridge picks daemon-or-in-process, so
//! these wrappers stay shape-only.
//!
//! Commands are `async` and run their work via [`run_off_main`] /
//! [`crate::bridge::call_off_main`]: synchronous commands execute on the
//! webview's main thread, so socket IPC or image work there freezes the UI
//! for the duration of the call.
//!
//! Split by responsibility — adding a new command means: write it in the
//! matching submodule, then list it in `lib.rs`'s `tauri::generate_handler!`
//! via its submodule path (e.g. `commands::library::library_list`).

use serde_json::Value;

use crate::errors::IpcError;

pub(crate) mod about;
pub(crate) mod autostart;
pub(crate) mod config;
pub(crate) mod daemon;
pub(crate) mod in_process;
pub(crate) mod library;
pub(crate) mod monitors;
pub(crate) mod preview;
pub(crate) mod profiles;
pub(crate) mod runtime;
pub(crate) mod slideshow;
pub(crate) mod tray;
pub(crate) mod window;

/// Run `work` on the blocking thread pool. A panicked or cancelled task
/// surfaces as an internal error instead of poisoning the command channel.
pub(crate) async fn run_off_main(
    work: impl FnOnce() -> Result<Value, IpcError> + Send + 'static,
) -> Result<Value, IpcError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| IpcError::internal(format!("command task failed: {e}")))?
}
