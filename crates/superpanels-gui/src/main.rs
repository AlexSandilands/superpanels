#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri shell entry point. The actual `tauri::Builder` lives in [`lib`].

/// Rust-side image decodes (canvas source thumbnails, daemon-unreachable
/// fallbacks) would otherwise stay parked in glibc's per-thread arenas after
/// free, inflating steady-state RSS (#82). Only covers Rust allocations —
/// `WebKitGTK` manages its own.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    superpanels_gui::run();
}
