#![forbid(unsafe_code)]

//! Tauri shell for Superpanels. The crate hosts:
//! - the `tauri::Builder` setup and lifecycle,
//! - typed `#[tauri::command]` wrappers for every IPC method,
//! - a daemon-first / in-process bridge that mirrors the CLI's behaviour,
//! - the system tray,
//! - autostart `.desktop` writer,
//! - desktop-entry / icon installer (taskbar icon on Wayland).
//!
//! Desktop-notification surfacing is intentionally
//! unimplemented; wire it back when the tray exposes failure events.

pub(crate) mod autostart;
pub(crate) mod bridge;
pub(crate) mod commands;
pub(crate) mod desktop_entry;
pub(crate) mod dmabuf;
pub(crate) mod errors;
pub(crate) mod state;
pub(crate) mod tray;
pub(crate) mod window_chrome;
pub(crate) mod window_state;

use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use crate::state::AppState;

/// Entry point. Spawns the Tauri runtime; never returns under normal use.
pub fn run() {
    init_tracing();
    // Re-exec (if warranted) before any thread spawns or the webview inits.
    dmabuf::apply();
    // Cap Tauri's async runtime before `build_app` runs: left to its default it
    // spawns one worker per core (64 idle threads on a 32-core box — GitHub
    // #84). `set` stores only the handle, so the `Runtime` must outlive the app;
    // `run()` blocks until exit, so this binding does.
    let _runtime = install_async_runtime();
    let start_hidden = wants_tray_mode(std::env::args().skip(1));
    build_app(start_hidden).run(handle_event);
}

/// Install a small tokio runtime as Tauri's async runtime, returning it so the
/// caller keeps it alive. On build failure we log and return `None`, letting
/// Tauri lazily create its default (per-core) runtime instead.
fn install_async_runtime() -> Option<tokio::runtime::Runtime> {
    match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(runtime) => {
            tauri::async_runtime::set(runtime.handle().clone());
            Some(runtime)
        }
        Err(error) => {
            tracing::warn!(%error, "could not build capped tokio runtime; using tauri default");
            None
        }
    }
}

/// Whether the GUI was launched in tray-only mode. The login-autostart entry
/// passes `--tray` so a session start brings up the tray + daemon without
/// popping the window; a normal launch (app menu) omits it and shows the window.
fn wants_tray_mode<I: IntoIterator<Item = String>>(args: I) -> bool {
    args.into_iter().any(|a| a == "--tray")
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Second-launch handler: a duplicate launch focuses the existing window
/// (unless it's an autostart `--tray` relaunch, which stays backgrounded)
/// instead of standing up a second tray + daemon, and recovers a dead daemon.
// reason: the `tauri_plugin_single_instance` callback signature is fixed
// (`Fn(&AppHandle, Vec<String>, String)`); we can't borrow these args.
#[allow(clippy::needless_pass_by_value)]
fn on_second_instance(app: &tauri::AppHandle, argv: Vec<String>, _cwd: String) {
    use tauri::Manager;
    if !argv.iter().any(|a| a == "--tray") {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
    std::thread::spawn(|| {
        if let Err(e) = crate::commands::daemon::ensure_daemon_running() {
            tracing::warn!(error = %e, "ensure daemon on relaunch failed");
        }
    });
}

/// Tauri `setup` hook: install the tray, restore + (conditionally) show the
/// window, and start the background workers and the daemon we own.
fn setup_app(
    app: &mut tauri::App,
    state: &Arc<AppState>,
    drag_regions: &crate::window_chrome::DragRegions,
    start_hidden: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    crate::tray::install(app, Arc::clone(state))?;
    crate::window_state::restore(app);
    // The window defaults to hidden (`visible: false`); a normal launch opts
    // into showing it, tray-only autostart leaves it hidden.
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = crate::window_chrome::install(&window, drag_regions) {
            tracing::warn!(error = %e, "window chrome not installed; the window may not move or resize");
        }
        if !start_hidden {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    crate::tray::spawn_poller(app.handle().clone(), Arc::clone(state));
    crate::commands::monitors::spawn_push_relay(app.handle().clone());
    // We own the daemon's lifecycle: bring it up on launch. Non-fatal — the
    // GUI shows a "daemon not running" banner if this fails.
    spawn_named("ensure-daemon", || {
        if let Err(e) = crate::commands::daemon::ensure_daemon_running() {
            tracing::warn!(error = %e, "could not ensure daemon at startup");
        }
    });
    spawn_named("desktop-entry", || {
        if let Err(e) = crate::desktop_entry::install() {
            tracing::warn!(error = %e, "desktop entry install failed");
        }
    });
    Ok(())
}

/// Close hides to tray instead of quitting: the process — and so the tray and
/// the daemon we own — stays alive. The real quit is tray → "Exit Superpanels".
fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    // Only the main window hides to tray; any future secondary window (dialog,
    // picker) should close normally rather than become un-closeable.
    if window.label() != "main" {
        return;
    }
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        // Persist geometry first so the next show restores it.
        let _ = crate::window_state::persist(window);
        api.prevent_close();
        let _ = window.hide();
    }
}

fn spawn_named(name: &str, work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(work)
        .ok();
}

fn build_app(start_hidden: bool) -> tauri::App {
    let state = Arc::new(AppState::new());
    let drag_regions = crate::window_chrome::DragRegions::default();

    let builder = tauri::Builder::default()
        // Single-instance must be registered first so a second launch routes
        // through its callback instead of building a duplicate app.
        .plugin(tauri_plugin_single_instance::init(on_second_instance))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::clone(&state))
        .manage(drag_regions.clone())
        .setup(move |app| setup_app(app, &state, &drag_regions, start_hidden))
        .on_window_event(on_window_event)
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
            commands::slideshow::slideshow_goto,
            commands::slideshow::slideshow_pause,
            commands::slideshow::slideshow_pool,
            commands::config::get_config,
            commands::config::save_config,
            commands::config::open_config_file,
            commands::about::open_release_page,
            commands::runtime::current_state,
            commands::autostart::set_autostart,
            commands::autostart::get_autostart,
            commands::tray::set_tray_icon_style,
            commands::tray::get_tray_icon_style,
            commands::daemon::daemon_status,
            commands::daemon::start_daemon,
            commands::window::set_drag_regions,
            commands::window::resize_bands,
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
            // We own the daemon's lifecycle, so it must not outlive this
            // process: stop it on every exit path (tray "Exit", a fatal error,
            // a relaunch teardown), not just the tray menu. Best-effort and
            // prompt — the daemon's `shutdown` handler takes no locks and
            // replies at once. `request_shutdown` then stops the tray poller
            // before the runtime tears down.
            let _ = crate::bridge::call(
                "shutdown",
                serde_json::json!({}),
                state.config_path().as_deref(),
            );
            state.request_shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wants_tray_mode;

    #[test]
    fn tray_mode_detected_only_when_flag_present() {
        assert!(wants_tray_mode(["--tray".to_owned()]));
        assert!(wants_tray_mode([
            "--verbose".to_owned(),
            "--tray".to_owned()
        ]));
        assert!(!wants_tray_mode(["--verbose".to_owned()]));
        assert!(!wants_tray_mode(Vec::<String>::new()));
        // A bare substring must not trigger it.
        assert!(!wants_tray_mode(["--tray-ish".to_owned()]));
    }
}
