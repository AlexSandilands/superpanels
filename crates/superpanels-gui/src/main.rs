#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri shell entry point. The actual `tauri::Builder` lives in [`lib`].

fn main() {
    superpanels_gui::run();
}
