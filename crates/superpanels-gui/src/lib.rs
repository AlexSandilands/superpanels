#![forbid(unsafe_code)]

//! Tauri shell for Superpanels (`SPEC.md` §12). The crate hosts:
//! - the `tauri::Builder` setup and lifecycle,
//! - typed `#[tauri::command]` wrappers for every IPC method (`SPEC.md` §12.4),
//! - a daemon-first / in-process bridge that mirrors the CLI's behaviour,
//! - the system tray (`SPEC.md` §13),
//! - desktop-notification surfacing (`SPEC.md` §13.4),
//! - autostart `.desktop` writer (`SPEC.md` §12 / §14).

pub mod autostart;
pub mod bridge;
pub mod commands;
pub mod errors;
pub mod ipc_client;
pub mod notifications;
pub mod state;
pub mod tray;
pub mod window_state;

use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use crate::state::AppState;

/// Entry point. Spawns the Tauri runtime; never returns under normal use.
pub fn run() {
    init_tracing();
    build_app().run(handle_event);
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn build_app() -> tauri::App {
    let state = Arc::new(AppState::new());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Arc::clone(&state))
        .setup(move |app| {
            crate::tray::install(app, Arc::clone(&state))?;
            crate::window_state::restore(app);
            crate::tray::spawn_poller(app.handle().clone(), Arc::clone(&state));
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let _ = crate::window_state::persist(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_monitors,
            commands::list_profiles,
            commands::apply_profile,
            commands::save_profile,
            commands::delete_profile,
            commands::preview_crop,
            commands::library_list,
            commands::library_thumbnail,
            commands::source_thumbnail,
            commands::library_tag,
            commands::library_delete,
            commands::library_rescan,
            commands::slideshow_next,
            commands::slideshow_prev,
            commands::slideshow_pause,
            commands::get_config,
            commands::save_config,
            commands::redetect,
            commands::current_state,
            commands::set_autostart,
            commands::get_autostart,
        ]);

    builder
        .build(tauri::generate_context!())
        .unwrap_or_else(|err| {
            tracing::error!(error = %err, "failed to build tauri app");
            std::process::exit(1);
        })
}

// reason: Tauri's run callback signature requires owned `RunEvent`.
#[allow(clippy::needless_pass_by_value)]
fn handle_event(_app: &tauri::AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::ExitRequested { code, .. } = &event {
        tracing::info!(code = ?code, "exit requested");
    }
}
