//! `#[tauri::command]` wrappers for the IPC surface in `SPEC.md` §12.4.
//!
//! Each command is a 3-line bridge: validate args → call [`crate::bridge`] →
//! return `Result<T, IpcError>`. The bridge picks daemon-or-in-process, so
//! these wrappers stay shape-only.
//!
//! Split by responsibility — adding a new command means: write it in the
//! matching submodule, then list it in `lib.rs`'s `tauri::generate_handler!`
//! via its submodule path (e.g. `commands::library::library_list`).

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
