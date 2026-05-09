//! Library `#[tauri::command]`s — list / thumbnail / source-thumbnail /
//! rescan / delete / tag (`SPEC.md` §7 / §12.4 / §17).

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
use crate::state::AppState;

/// Filter forwarded to `library_list`. Mirrors `superpanels_core::LibraryFilter`
/// but is defined in the GUI crate so `ts-rs` can export it for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub(crate) struct LibraryFilter {
    #[ts(optional)]
    pub(crate) tag: Option<String>,
    #[ts(optional)]
    pub(crate) min_width: Option<u32>,
    #[ts(optional)]
    pub(crate) min_height: Option<u32>,
    #[ts(optional)]
    pub(crate) aspect_min: Option<f32>,
    #[ts(optional)]
    pub(crate) aspect_max: Option<f32>,
    #[ts(optional)]
    pub(crate) offset: Option<u32>,
    #[ts(optional)]
    pub(crate) limit: Option<u32>,
}

#[tauri::command]
pub(crate) fn library_list(
    filter: LibraryFilter,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let params = serde_json::to_value(&filter)
        .map_err(|e| IpcError::internal(format!("LibraryFilter serialise: {e}")))?;
    bridge::call("library_list", params, state.config_path().as_deref())
}

#[tauri::command]
pub(crate) fn library_thumbnail(
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
pub(crate) fn source_thumbnail(path: String) -> Result<Value, IpcError> {
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(IpcError::invalid("source thumbnail path must be absolute"));
    }
    render_local_thumbnail(&p)
}

fn render_local_thumbnail(path: &std::path::Path) -> Result<Value, IpcError> {
    // Matches `LibraryConfig::thumbnail_size`'s default; the canvas preview
    // doesn't have config in scope so the constant is hard-coded here.
    const THUMBNAIL_MAX_EDGE: u32 = 512;

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
pub(crate) fn library_rescan(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call("library_rescan", json!({}), state.config_path().as_deref())
}

#[tauri::command]
pub(crate) fn library_delete(
    path: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call(
        "library_delete",
        json!({ "path": path }),
        state.config_path().as_deref(),
    )
}

#[tauri::command]
pub(crate) fn library_tag(
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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same
mod tests {
    use super::*;

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
