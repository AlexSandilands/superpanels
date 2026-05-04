//! `#[tauri::command]` wrappers for the IPC surface in `SPEC.md` §12.4.
//!
//! Each command is a 3-line bridge: validate args → call [`crate::bridge`] →
//! return `Result<T, IpcError>`. The bridge picks daemon-or-in-process, so
//! these wrappers stay shape-only.

// reason: Tauri requires owned `String`, `Value`, and `tauri::State` types
// in `#[tauri::command]` signatures (the macro deserialises them from JSON).
// `needless_pass_by_value` fires on every command and the suggested `&_`
// signatures don't compile with the macro.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::warn;
use ts_rs::TS;

use crate::bridge;
use crate::errors::IpcError;
use crate::state::{AppState, RuntimeSnapshot};

pub mod in_process;

// --- shared payload types (TS-exported) --------------------------------------

/// Filter forwarded to `library_list`. Mirrors `superpanels_core::LibraryFilter`
/// but is defined in the GUI crate so `ts-rs` can export it for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct LibraryFilter {
    #[ts(optional)]
    pub tag: Option<String>,
    #[ts(optional)]
    pub min_width: Option<u32>,
    #[ts(optional)]
    pub min_height: Option<u32>,
    #[ts(optional)]
    pub aspect_min: Option<f32>,
    #[ts(optional)]
    pub aspect_max: Option<f32>,
    #[ts(optional)]
    pub offset: Option<u32>,
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct PreviewArgs {
    pub image: String,
    pub offset_px: [i32; 2],
    pub bezel_h_mm: f32,
    pub bezel_v_mm: f32,
    pub fit: String,
}

// --- command bindings --------------------------------------------------------

#[tauri::command]
pub fn detect_monitors(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("redetect", json!({}), state.config_path().as_deref())?;
    bridge::call("detect_monitors", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn list_profiles(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("list_profiles", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn apply_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    if name.trim().is_empty() {
        return Err(IpcError::invalid("profile name is empty"));
    }
    bridge::call(
        "apply_profile",
        json!({ "name": name }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn save_profile(
    profile: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "save_profile",
        json!({ "profile": profile }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn delete_profile(
    name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "delete_profile",
        json!({ "name": name }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn preview_crop(
    args: PreviewArgs,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "preview_crop",
        serde_json::to_value(&args).unwrap_or(Value::Null),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn library_list(
    filter: LibraryFilter,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "library_list",
        serde_json::to_value(&filter).unwrap_or(Value::Null),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn library_thumbnail(
    path: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(IpcError::invalid("thumbnail path must be absolute"));
    }
    let result = bridge::call(
        "library_thumbnail",
        json!({ "path": p.to_string_lossy() }),
        state.config_path().as_deref(),
    );
    match result {
        Ok(v) => Ok(v),
        Err(e) if should_fall_back_to_local_render(&e) => {
            warn!(error = %e, "library_thumbnail daemon unreachable; falling back to local render");
            render_local_thumbnail(&p)
        }
        Err(e) => Err(e),
    }
}

/// Whether `library_thumbnail` is allowed to fall back to a local render in
/// response to `err`. Only `IpcError::DaemonUnreachable` qualifies — every
/// other variant (including `IpcError::Daemon`, the daemon's logical
/// rejections) propagates unchanged so the library-roots gate cannot be
/// bypassed by crafting a path whose error message contains a transport
/// keyword (`SPEC §17`, confused-deputy guard).
fn should_fall_back_to_local_render(err: &IpcError) -> bool {
    matches!(err, IpcError::DaemonUnreachable(_))
}

#[tauri::command]
pub fn source_thumbnail(path: String) -> Result<Value, IpcError> {
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(IpcError::invalid("source thumbnail path must be absolute"));
    }
    render_local_thumbnail(&p)
}

fn render_local_thumbnail(path: &std::path::Path) -> Result<Value, IpcError> {
    const THUMBNAIL_MAX_EDGE: u32 = 320;

    let canonical = std::fs::canonicalize(path)
        .map_err(|e| IpcError::invalid(format!("rejecting path '{}': {e}", path.display())))?;
    let img = superpanels_core::image::load_thumbnail(&canonical, THUMBNAIL_MAX_EDGE)
        .map_err(|e| IpcError::Image(e.to_string()))?;
    let bytes =
        superpanels_core::image::encode_png(&img).map_err(|e| IpcError::Image(e.to_string()))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(json!({ "data": encoded, "mime": "image/png" }))
}

#[tauri::command]
pub fn library_tag(
    path: String,
    tag: String,
    on: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "library_tag",
        json!({ "path": path, "tag": tag, "on": on }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn slideshow_next(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("slideshow_next", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn slideshow_prev(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("slideshow_prev", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn slideshow_pause(
    paused: Option<bool>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let mut params = json!({});
    if let Some(p) = paused {
        params = json!({ "paused": p });
    }
    bridge::call("slideshow_pause", params, state.config_path().as_deref())
}

#[tauri::command]
pub fn get_config(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("get_config", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn save_config(
    config: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "save_config",
        json!({ "config": config }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub fn redetect(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("redetect", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub fn current_state(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    let v = bridge::call("current_state", json!({}), state.config_path().as_deref())?;
    state.set_snapshot(parse_runtime_snapshot(&v));
    Ok(v)
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<Value, IpcError> {
    crate::autostart::set_enabled(enabled).map(|()| json!({ "enabled": enabled }))
}

#[tauri::command]
pub fn get_autostart() -> Result<Value, IpcError> {
    Ok(json!({ "enabled": crate::autostart::is_enabled() }))
}

fn parse_runtime_snapshot(v: &Value) -> RuntimeSnapshot {
    let active_profile = v
        .get("active_profile")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let paused = v
        .get("slideshow")
        .and_then(|s| s.get("paused"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let current_filename = v
        .get("current_filename")
        .and_then(Value::as_str)
        .map(str::to_owned);
    RuntimeSnapshot {
        active_profile,
        current_filename,
        paused,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_snapshot_handles_minimal_payload() {
        let v = json!({"active_profile": "home", "slideshow": {"paused": true}});
        let snap = parse_runtime_snapshot(&v);
        assert_eq!(snap.active_profile.as_deref(), Some("home"));
        assert!(snap.paused);
    }

    #[test]
    fn parse_runtime_snapshot_defaults_when_fields_missing() {
        let v = json!({});
        let snap = parse_runtime_snapshot(&v);
        assert!(snap.active_profile.is_none());
        assert!(!snap.paused);
    }

    #[test]
    fn render_local_thumbnail_returns_base64_png_for_arbitrary_path() {
        // Drives the Tauri-wrapper fallback path used by `library_thumbnail`
        // and the canvas's `source_thumbnail`. Locks in that any decodable
        // image — including one outside library roots — round-trips to a
        // non-empty base64 PNG payload the webview can render.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("thumb-input.png");
        let img = image::RgbImage::from_pixel(64, 48, image::Rgb([255, 128, 64]));
        img.save(&path).expect("write fixture png");

        let value = render_local_thumbnail(&path).expect("render local thumbnail");
        let data = value
            .get("data")
            .and_then(Value::as_str)
            .expect("data field present");
        let mime = value
            .get("mime")
            .and_then(Value::as_str)
            .expect("mime field present");
        assert_eq!(mime, "image/png");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .expect("base64 decodes");
        // PNG magic — any successful encode must start with these bytes.
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn source_thumbnail_rejects_relative_path() {
        // Mirrors `library_thumbnail`'s absolute-path requirement so the
        // Rust side can't be tricked into resolving against the daemon's
        // working directory.
        let err = source_thumbnail("relative/path.png".to_owned()).unwrap_err();
        assert!(matches!(err, IpcError::InvalidArgument(_)));
    }

    #[test]
    fn library_thumbnail_fallback_only_fires_on_daemon_unreachable() {
        // Confused-deputy guard for `library_thumbnail`'s fallback
        // (`SPEC §17`): the local render must only run when the bridge
        // signals the daemon was unreachable. Every other `IpcError` —
        // including `Daemon(_)` carrying the daemon's logical rejection of
        // a user-supplied path — must propagate unchanged so the roots
        // gate cannot be bypassed.
        //
        // The round-1 substring classifier was bypassed by paths whose
        // contents echoed transport keywords (e.g. `/tmp/socket-bait.png`
        // matched `.contains("socket")`). Round 2 replaces it with a
        // structural variant; the table below pins that — under a
        // path-pattern fuzz we'd never need string matches at all.
        assert!(should_fall_back_to_local_render(
            &IpcError::DaemonUnreachable("connection refused".to_owned())
        ));
        assert!(should_fall_back_to_local_render(
            &IpcError::DaemonUnreachable(
                "no daemon is running — start one with `superpanels-daemon`".to_owned()
            )
        ));

        // Logical rejections from the daemon — even when the daemon's error
        // string echoes a transport keyword via the user-supplied path —
        // must NOT trigger the fallback. These are the round-1 bypass
        // cases; they should now be inert by construction.
        for daemon_msg in [
            "path '/etc/passwd' is outside the configured library roots",
            "path '/tmp/socket-bait.png' is outside the configured library roots",
            "path '/etc/connection-test/x.png' is outside the configured library roots",
            "path '/srv/eof-marker/y.jpg' is outside the configured library roots",
            "path '/var/transport-cache/z.webp' is outside the configured library roots",
            "path '/home/user/no daemon/img.png' is outside the configured library roots",
            "image: failed to decode bytes",
        ] {
            assert!(
                !should_fall_back_to_local_render(&IpcError::Daemon(daemon_msg.to_owned())),
                "logical Daemon rejection routed to fallback: {daemon_msg}"
            );
        }

        // Non-Daemon variants never trigger the fallback either.
        assert!(!should_fall_back_to_local_render(&IpcError::invalid(
            "bad path"
        )));
        assert!(!should_fall_back_to_local_render(&IpcError::Library(
            "scan failed".to_owned()
        )));
        assert!(!should_fall_back_to_local_render(&IpcError::Image(
            "decode failed".to_owned()
        )));
    }
}
