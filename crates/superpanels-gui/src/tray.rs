//! System tray (`SPEC.md` §13). Builds an initial menu, polls daemon state
//! once per second to keep the active-profile tick mark in sync, and routes
//! menu events to the right Tauri command (or quits the app).

// reason: the tray needs to keep `Arc<AppState>` alive inside long-lived
// closures and a background thread. The `needless_pass_by_value` lint is
// noisy here; ownership is genuine.
#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, Wry};

use crate::bridge;
use crate::state::{AppState, RuntimeSnapshot};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

const ID_NEXT: &str = "tray.slideshow.next";
const ID_PREV: &str = "tray.slideshow.prev";
const ID_PAUSE: &str = "tray.slideshow.pause";
const ID_OPEN: &str = "tray.open";
const ID_SETTINGS: &str = "tray.settings";
const ID_QUIT: &str = "tray.quit";
const PROFILE_PREFIX: &str = "tray.profile.";

pub fn install(app: &App, state: Arc<AppState>) -> tauri::Result<()> {
    let handle = app.handle().clone();
    let menu = build_initial_menu(&handle, &state)?;
    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon_image())
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Superpanels")
        .on_menu_event({
            let handle = handle.clone();
            let state = Arc::clone(&state);
            move |app, event| handle_menu_event(app, &event.id().0, &handle, &state)
        })
        .on_tray_icon_event({
            let handle = handle.clone();
            move |_tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    toggle_main_window(&handle);
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Background poller that re-fetches `current_state` and rebuilds the tray
/// menu when the active profile or pause flag changes. Keeps the tick mark
/// and "Pause / Resume" label in sync without the user re-opening the menu.
pub fn spawn_poller(handle: AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("tray-poller".into())
        .spawn(move || {
            let mut last_signature: Option<String> = None;
            loop {
                std::thread::sleep(POLL_INTERVAL);
                refresh_snapshot(&state);
                let snap = state.snapshot();
                let sig = format!(
                    "{}|{}|{}",
                    snap.active_profile.clone().unwrap_or_default(),
                    snap.current_filename.clone().unwrap_or_default(),
                    snap.paused
                );
                if last_signature.as_deref() == Some(sig.as_str()) {
                    continue;
                }
                if let Err(e) = rebuild_menu(&handle, &state) {
                    tracing::warn!(error = %e, "tray menu rebuild failed");
                } else {
                    last_signature = Some(sig);
                }
            }
        })
        .ok();
}

fn tray_icon_image() -> tauri::image::Image<'static> {
    // PNG bytes embedded at build time. Falls back to a 1x1 transparent
    // image only if decode fails, so tray construction can never panic.
    const TRAY_PNG: &[u8] = include_bytes!("../icons/tray.png");
    tauri::image::Image::from_bytes(TRAY_PNG)
        .unwrap_or_else(|_| tauri::image::Image::new_owned(vec![0; 4], 1, 1))
}

fn refresh_snapshot(state: &Arc<AppState>) {
    let cfg_path = state.config_path();
    if let Ok(v) = bridge::call("current_state", json!({}), cfg_path.as_deref()) {
        let active = v
            .get("active_profile")
            .and_then(|x| x.as_str())
            .map(str::to_owned);
        let paused = v
            .get("slideshow")
            .and_then(|s| s.get("paused"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let filename = v
            .get("current_filename")
            .and_then(|x| x.as_str())
            .map(str::to_owned);
        state.set_snapshot(RuntimeSnapshot {
            active_profile: active,
            current_filename: filename,
            paused,
        });
    }
}

fn list_profile_names(state: &Arc<AppState>) -> Vec<String> {
    bridge::call("list_profiles", json!({}), state.config_path().as_deref())
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.into_iter()
                .filter_map(|p| p.get("name").and_then(|n| n.as_str()).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn build_initial_menu(handle: &AppHandle, state: &Arc<AppState>) -> tauri::Result<Menu<Wry>> {
    let snap = state.snapshot();
    let profiles = list_profile_names(state);
    build_menu(handle, &profiles, &snap)
}

fn rebuild_menu(handle: &AppHandle, state: &Arc<AppState>) -> tauri::Result<()> {
    let snap = state.snapshot();
    let profiles = list_profile_names(state);
    let menu = build_menu(handle, &profiles, &snap)?;
    if let Some(tray) = handle.tray_by_id("main-tray") {
        tray.set_menu(Some(menu))?;
        tray.set_tooltip(Some(tooltip_for(&snap)))?;
    }
    Ok(())
}

fn tooltip_for(snap: &RuntimeSnapshot) -> String {
    let profile = snap.active_profile.as_deref().unwrap_or("(no profile)");
    match snap.current_filename.as_deref() {
        Some(file) => format!("Superpanels — {profile} — {file}"),
        None => format!("Superpanels — {profile}"),
    }
}

fn build_menu(
    handle: &AppHandle,
    profiles: &[String],
    snap: &RuntimeSnapshot,
) -> tauri::Result<Menu<Wry>> {
    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<Wry>>> = Vec::new();

    // Profile section ----------------------------------------------------
    if profiles.is_empty() {
        let placeholder = MenuItem::with_id(
            handle,
            "tray.profile.empty",
            "(no profiles)",
            false,
            None::<&str>,
        )?;
        items.push(Box::new(placeholder));
    } else {
        for name in profiles {
            let id = format!("{PROFILE_PREFIX}{name}");
            let checked = snap.active_profile.as_deref() == Some(name.as_str());
            let item = CheckMenuItem::with_id(handle, &id, name, true, checked, None::<&str>)?;
            items.push(Box::new(item));
        }
    }

    items.push(Box::new(PredefinedMenuItem::separator(handle)?));

    // Slideshow controls -------------------------------------------------
    items.push(Box::new(MenuItem::with_id(
        handle,
        ID_NEXT,
        "Next",
        true,
        None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(
        handle,
        ID_PREV,
        "Previous",
        true,
        None::<&str>,
    )?));
    let pause_label = if snap.paused {
        "Resume slideshow"
    } else {
        "Pause slideshow"
    };
    items.push(Box::new(MenuItem::with_id(
        handle,
        ID_PAUSE,
        pause_label,
        true,
        None::<&str>,
    )?));

    items.push(Box::new(PredefinedMenuItem::separator(handle)?));

    items.push(Box::new(MenuItem::with_id(
        handle,
        ID_OPEN,
        "Open Superpanels",
        true,
        None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(
        handle,
        ID_SETTINGS,
        "Settings…",
        true,
        None::<&str>,
    )?));

    items.push(Box::new(PredefinedMenuItem::separator(handle)?));
    items.push(Box::new(MenuItem::with_id(
        handle,
        ID_QUIT,
        "Quit",
        true,
        None::<&str>,
    )?));

    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = items.iter().map(AsRef::as_ref).collect();
    Menu::with_items(handle, &refs)
}

fn handle_menu_event(app: &AppHandle, id: &str, _handle: &AppHandle, state: &Arc<AppState>) {
    match id {
        ID_QUIT => app.exit(0),
        ID_OPEN => show_main_window(app),
        ID_SETTINGS => {
            show_main_window(app);
            let _ = app.emit("tray://open-settings", ());
        }
        ID_NEXT => {
            let _ = bridge::call("slideshow_next", json!({}), state.config_path().as_deref());
        }
        ID_PREV => {
            let _ = bridge::call("slideshow_prev", json!({}), state.config_path().as_deref());
        }
        ID_PAUSE => {
            let snap = state.snapshot();
            let new_paused = !snap.paused;
            let _ = bridge::call(
                "slideshow_pause",
                json!({ "paused": new_paused }),
                state.config_path().as_deref(),
            );
        }
        other if other.starts_with(PROFILE_PREFIX) => {
            let name = other.trim_start_matches(PROFILE_PREFIX).to_owned();
            if !name.is_empty() && name != "empty" {
                let _ = bridge::call(
                    "apply_profile",
                    json!({ "name": name }),
                    state.config_path().as_deref(),
                );
            }
        }
        _ => {}
    }
}

fn toggle_main_window(handle: &AppHandle) {
    let Some(window) = handle.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn show_main_window(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(active: Option<&str>, file: Option<&str>, paused: bool) -> RuntimeSnapshot {
        RuntimeSnapshot {
            active_profile: active.map(str::to_owned),
            current_filename: file.map(str::to_owned),
            paused,
        }
    }

    #[test]
    fn tooltip_includes_profile_only_when_no_filename() {
        let s = snap(Some("home"), None, false);
        assert_eq!(tooltip_for(&s), "Superpanels — home");
    }

    #[test]
    fn tooltip_includes_profile_and_filename_when_present() {
        let s = snap(Some("home"), Some("a.png"), false);
        assert_eq!(tooltip_for(&s), "Superpanels — home — a.png");
    }

    #[test]
    fn tooltip_falls_back_when_no_active_profile() {
        let s = snap(None, None, false);
        assert_eq!(tooltip_for(&s), "Superpanels — (no profile)");
    }
}
