//! Main-window teardown and rebuild. Closing the window destroys its webview so
//! the `WebKit` processes exit while the app sits in the tray; tray "Open" /
//! "Settings" and a second launch rebuild it from the bundled config.

use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};

use crate::window_chrome::DragRegions;

pub(crate) const MAIN_LABEL: &str = "main";

/// What an `ExitRequested` should do, decided purely from its exit code.
///
/// Destroying the last window makes the tauri runtime fire `ExitRequested` with
/// `code: None`; the tray "Exit" item calls `AppHandle::exit`, which fires it
/// with `code: Some(_)`. Only the latter is a real quit that must stop the
/// daemon we own — the former is a close-to-tray that has to stay resident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitDecision {
    /// A deliberate quit (tray "Exit", programmatic exit/restart): stop the
    /// daemon and let the process exit.
    Quit,
    /// The last window closed to tray: stay alive, keep the daemon running.
    StayResident,
}

pub(crate) fn exit_decision(code: Option<i32>) -> ExitDecision {
    match code {
        Some(_) => ExitDecision::Quit,
        None => ExitDecision::StayResident,
    }
}

/// Tear the main window down to tray: persist geometry, then `destroy()` to free
/// the webview and its `WebKit` processes. Shared by the close-button path and
/// the tray-icon dismiss so both reclaim the ~300MB rather than just hiding.
pub(crate) fn tear_down_to_tray(window: &WebviewWindow) {
    let _ = crate::window_state::persist(window);
    let _ = window.destroy();
}

/// Show the main window, rebuilding its webview from config first when a prior
/// close-to-tray tore it down.
///
/// Always hops to the GTK main thread before touching windows: reached from
/// [`crate::on_second_instance`] this runs on the single-instance plugin's zbus
/// handler thread, and the rebuild wires GTK signal handlers
/// ([`crate::window_chrome::install`]) that are main-thread-only.
pub(crate) fn show_or_recreate_main_window(app: &AppHandle) {
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || show_or_recreate_on_main(&handle)) {
        tracing::error!(error = %e, "could not dispatch main-window open to the main thread");
    }
}

fn show_or_recreate_on_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    match recreate_main_window(app) {
        Ok(window) => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        Err(e) => tracing::error!(error = %e, "could not rebuild the main window"),
    }
}

fn recreate_main_window(app: &AppHandle) -> anyhow::Result<WebviewWindow> {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_LABEL)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no '{MAIN_LABEL}' window in the bundled tauri config"))?;
    let window = WebviewWindowBuilder::from_config(app, &window_config)?.build()?;
    configure_main_window(app, &window);
    Ok(window)
}

/// Install the native window chrome and restore saved geometry. Shared by
/// first-launch setup and the post-teardown rebuild so both windows behave the
/// same. The chrome handlers hang off the window's own GTK widgets, so a rebuilt
/// window gets a fresh set — nothing to unregister from the destroyed one.
pub(crate) fn configure_main_window(app: &AppHandle, window: &WebviewWindow) {
    if let Some(regions) = app.try_state::<DragRegions>() {
        if let Err(e) = crate::window_chrome::install(window, &regions) {
            tracing::warn!(error = %e, "window chrome not installed; the window may not move or resize");
        }
    }
    crate::window_state::restore(window);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_code_stays_resident_some_code_quits() {
        // Last-window teardown → runtime fires `code: None` → tray app lives on.
        assert_eq!(exit_decision(None), ExitDecision::StayResident);
        // Tray "Exit" / programmatic exit → `code: Some(_)` → real quit.
        assert_eq!(exit_decision(Some(0)), ExitDecision::Quit);
        assert_eq!(exit_decision(Some(1)), ExitDecision::Quit);
        // Restart uses a sentinel non-zero code; it must still tear the daemon
        // down before the process re-execs.
        assert_eq!(exit_decision(Some(i32::MIN)), ExitDecision::Quit);
    }
}
