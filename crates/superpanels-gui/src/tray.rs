//! System tray (`SPEC.md` §13). Builds an initial menu, polls daemon state
//! to keep the active-profile tick mark in sync, and routes menu events to
//! the right Tauri command (or quits the app).

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

// Tray sync runs on a background thread; the user only feels this cadence on
// slideshow advance / pause label updates, and `menu_signature` already
// suppresses no-op rebuilds. 5s matches the App.svelte refresh cadence so
// tray and main window stay loosely in step.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

const ID_NEXT: &str = "tray.slideshow.next";
const ID_PREV: &str = "tray.slideshow.prev";
const ID_PAUSE: &str = "tray.slideshow.pause";
const ID_OPEN: &str = "tray.open";
const ID_SETTINGS: &str = "tray.settings";
const ID_QUIT: &str = "tray.quit";
const PROFILE_PREFIX: &str = "tray.profile.";

pub(crate) fn install(app: &App, state: Arc<AppState>) -> tauri::Result<()> {
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

/// Background poller that re-fetches `current_state` and updates the tray's
/// menu and tooltip when their respective inputs change. Keeps the active-
/// profile tick mark and "Pause / Resume" label in sync without the user
/// re-opening the menu.
///
/// Menu and tooltip have **separate cache keys** so that a slideshow advance
/// (which changes only `current_filename`) updates the tooltip but does not
/// trigger an `org.kde.StatusNotifierItem` `set_menu` round-trip — the
/// dominant per-tick cost on KDE Plasma.
///
/// Exits within one `POLL_INTERVAL` after [`AppState::request_shutdown`] is
/// called, so it doesn't outlive the Tauri runtime on `ExitRequested`.
pub(crate) fn spawn_poller(handle: AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("tray-poller".into())
        .spawn(move || {
            let mut last_menu_sig: Option<String> = None;
            let mut last_tooltip_sig: Option<String> = None;
            while !state.shutting_down() {
                std::thread::sleep(POLL_INTERVAL);
                if state.shutting_down() {
                    break;
                }
                refresh_snapshot(&state);
                let snap = state.snapshot();
                let profiles = list_profile_names(&state);

                let menu_sig = menu_signature(&snap, &profiles);
                if last_menu_sig.as_deref() != Some(menu_sig.as_str()) {
                    if let Err(e) = apply_menu(&handle, &profiles, &snap) {
                        tracing::warn!(error = %e, "tray menu rebuild failed");
                    } else {
                        last_menu_sig = Some(menu_sig);
                    }
                }

                let tooltip = tooltip_for(&snap);
                if last_tooltip_sig.as_deref() != Some(tooltip.as_str()) {
                    if let Err(e) = apply_tooltip(&handle, &tooltip) {
                        tracing::warn!(error = %e, "tray tooltip update failed");
                    } else {
                        last_tooltip_sig = Some(tooltip);
                    }
                }
            }
        })
        .ok();
}

fn menu_signature(snap: &RuntimeSnapshot, profiles: &[String]) -> String {
    // Newline separator can't appear inside profile names — `validate_profiles`
    // rejects control chars (`SPEC §17`), so it's safe as a delimiter.
    format!(
        "{}|{}|{}",
        snap.active_profile.as_deref().unwrap_or(""),
        snap.paused,
        profiles.join("\n"),
    )
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

fn apply_menu(
    handle: &AppHandle,
    profiles: &[String],
    snap: &RuntimeSnapshot,
) -> tauri::Result<()> {
    let menu = build_menu(handle, profiles, snap)?;
    if let Some(tray) = handle.tray_by_id("main-tray") {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

fn apply_tooltip(handle: &AppHandle, tooltip: &str) -> tauri::Result<()> {
    if let Some(tray) = handle.tray_by_id("main-tray") {
        tray.set_tooltip(Some(tooltip))?;
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
            if let Some(name) = parse_profile_menu_id(other) {
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

/// Pure parser for `tray.profile.<name>` menu IDs.
///
/// Returns the trimmed profile name when `id` starts with [`PROFILE_PREFIX`],
/// the suffix is non-empty, and is not the literal `"empty"` placeholder used
/// for the disabled "(no profiles)" menu item. Other inputs return `None`.
///
/// Extracted from `handle_menu_event` so the prefix logic can be unit-tested
/// without standing up a Tauri `AppHandle`.
fn parse_profile_menu_id(id: &str) -> Option<String> {
    let suffix = id.strip_prefix(PROFILE_PREFIX)?;
    if suffix.is_empty() || suffix == "empty" {
        return None;
    }
    Some(suffix.to_owned())
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
    fn menu_signature_ignores_filename_changes() {
        // Slideshow advance is the hot case — filename changes every tick but
        // the menu structure (profiles, active tick mark, pause label) stays
        // identical. Locking this invariant keeps the per-tick `set_menu`
        // DBus call from firing during normal slideshow playback.
        let profiles = vec!["home".to_owned(), "work".to_owned()];
        let a = snap(Some("home"), Some("a.png"), false);
        let b = snap(Some("home"), Some("b.png"), false);
        assert_eq!(menu_signature(&a, &profiles), menu_signature(&b, &profiles));
    }

    #[test]
    fn menu_signature_changes_when_active_profile_or_pause_or_list_changes() {
        let base = snap(Some("home"), Some("a.png"), false);
        let profiles = vec!["home".to_owned(), "work".to_owned()];
        let baseline = menu_signature(&base, &profiles);

        let switched = snap(Some("work"), Some("a.png"), false);
        assert_ne!(menu_signature(&switched, &profiles), baseline);

        let paused = snap(Some("home"), Some("a.png"), true);
        assert_ne!(menu_signature(&paused, &profiles), baseline);

        let renamed = vec!["house".to_owned(), "work".to_owned()];
        assert_ne!(menu_signature(&base, &renamed), baseline);

        let added = vec!["home".to_owned(), "work".to_owned(), "travel".to_owned()];
        assert_ne!(menu_signature(&base, &added), baseline);
    }

    #[test]
    fn parse_profile_menu_id_extracts_name_after_prefix() {
        assert_eq!(
            parse_profile_menu_id("tray.profile.home").as_deref(),
            Some("home")
        );
        // Profile names with dots / unicode survive — trim_start_matches would
        // have eaten matching chars, but `strip_prefix` only matches once.
        assert_eq!(
            parse_profile_menu_id("tray.profile.家.work").as_deref(),
            Some("家.work")
        );
    }

    #[test]
    fn parse_profile_menu_id_rejects_empty_and_placeholder() {
        // Empty suffix happens if the prefix is the whole id; `"empty"` is the
        // disabled "(no profiles)" placeholder we install when the profile
        // list is empty (see `build_menu`).
        assert!(parse_profile_menu_id("tray.profile.").is_none());
        assert!(parse_profile_menu_id("tray.profile.empty").is_none());
    }

    #[test]
    fn parse_profile_menu_id_rejects_unrelated_ids() {
        // Other tray menu IDs must not accidentally route through the
        // profile-apply branch — `apply_profile` with `name="quit"` would be a
        // very surprising consequence of clicking Quit.
        for id in [
            ID_NEXT,
            ID_PREV,
            ID_PAUSE,
            ID_OPEN,
            ID_SETTINGS,
            ID_QUIT,
            "tray.profileXhome",
            "",
        ] {
            assert!(
                parse_profile_menu_id(id).is_none(),
                "unexpected match for {id}"
            );
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
