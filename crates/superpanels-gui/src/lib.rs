#![forbid(unsafe_code)]

//! Tauri shell for Superpanels. The crate hosts:
//! - the `tauri::Builder` setup and lifecycle,
//! - typed `#[tauri::command]` wrappers for every IPC method,
//! - a daemon-first / in-process bridge that mirrors the CLI's behaviour,
//! - the system tray,
//! - autostart `.desktop` writer.
//!
//! Desktop-notification surfacing is intentionally
//! unimplemented; wire it back when the tray exposes failure events.

pub(crate) mod autostart;
pub(crate) mod bridge;
pub(crate) mod commands;
pub(crate) mod errors;
pub(crate) mod state;
pub(crate) mod tray;
pub(crate) mod window_state;

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
            crate::commands::monitors::spawn_push_relay(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let _ = crate::window_state::persist(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::monitors::detect_monitors,
            commands::monitors::redetect,
            commands::monitors::set_monitor_physical_size,
            commands::profiles::list_profiles,
            commands::profiles::apply_profile,
            commands::profiles::apply_canvas,
            commands::profiles::save_profile,
            commands::profiles::delete_profile,
            commands::profiles::duplicate_profile,
            commands::profiles::rename_profile,
            commands::profiles::update_profile_monitor_state,
            commands::profiles::update_profile_image_transform,
            commands::profiles::update_profile_source,
            commands::profiles::list_schedules,
            commands::profiles::save_schedules,
            commands::profiles::set_schedules_paused,
            commands::preview::preview_crop,
            commands::library::library_list,
            commands::library::library_thumbnail,
            commands::library::source_thumbnail,
            commands::library::library_tag,
            commands::library::library_delete,
            commands::library::library_rescan,
            commands::slideshow::slideshow_next,
            commands::slideshow::slideshow_prev,
            commands::slideshow::slideshow_pause,
            commands::config::get_config,
            commands::config::save_config,
            commands::runtime::current_state,
            commands::autostart::set_autostart,
            commands::autostart::get_autostart,
            commands::daemon::daemon_status,
            commands::daemon::start_daemon,
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
fn handle_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::ExitRequested { code, .. } = &event {
        use tauri::Manager;
        tracing::info!(code = ?code, "exit requested");
        if let Some(state) = app.try_state::<Arc<AppState>>() {
            state.request_shutdown();
        }
    }
}
