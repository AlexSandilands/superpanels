//! Library `#[tauri::command]`s — list / thumbnail / source-thumbnail /
//! rescan / delete / tag.

#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use superpanels_core::config::{DEFAULT_THUMBNAIL_SIZE, THUMBNAIL_MIN_EDGE};
use superpanels_core::image::Resample;
use tracing::warn;
use ts_rs::TS;

use super::run_off_main;
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
pub(crate) async fn library_list(
    filter: LibraryFilter,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let params = serde_json::to_value(&filter)
        .map_err(|e| IpcError::internal(format!("LibraryFilter serialise: {e}")))?;
    bridge::call_off_main("library_list", params, state.config_path()).await
}

// Thumbnail commands are async: the decode / IPC runs on a blocking thread
// via `spawn_blocking`, never on the webview's main thread. Synchronous
// commands block the UI, which made menus stutter for the duration of one
// full image decode per profile.
#[tauri::command]
pub(crate) async fn library_thumbnail(
    path: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(IpcError::invalid("thumbnail path must be absolute"));
    }
    let config_path = state.config_path();
    run_off_main(move || {
        let result = bridge::call(
            "library_thumbnail",
            json!({ "path": p.to_string_lossy() }),
            config_path.as_deref(),
        );
        match result {
            Ok(v) => Ok(v),
            Err(e) if should_fall_back_to_local_render(&e) => {
                warn!(error = %e, "library_thumbnail daemon unreachable; falling back to local render");
                render_local_thumbnail(&p, DEFAULT_THUMBNAIL_SIZE, Resample::Fast)
            }
            Err(e) => Err(e),
        }
    })
    .await
}

/// Whether `library_thumbnail` is allowed to fall back to a local render in
/// response to `err`. Only `IpcError::DaemonUnreachable` qualifies — every
/// other variant (including `IpcError::Daemon`, the daemon's logical
/// rejections) propagates unchanged so the library-roots gate cannot be
/// bypassed by crafting a path whose error message contains a transport
/// keyword. (Confused-deputy guard.)
fn should_fall_back_to_local_render(err: &IpcError) -> bool {
    matches!(err, IpcError::DaemonUnreachable(_))
}

/// Upper bound on `source_thumbnail`'s caller-chosen `max_edge`. The frontend
/// asks for a bigger edge on the preview canvas than in a grid tile, but the
/// value sizes a resample buffer and a base64 IPC payload, so it is clamped
/// rather than trusted: 2048px is ~9 MiB of base64 for an incompressible
/// source. `THUMBNAIL_MIN_EDGE` is the lower bound.
const SOURCE_THUMBNAIL_MAX_EDGE: u32 = 2048;

#[tauri::command]
pub(crate) async fn source_thumbnail(
    path: String,
    max_edge: Option<u32>,
) -> Result<Value, IpcError> {
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(IpcError::invalid("source thumbnail path must be absolute"));
    }
    let edge = clamp_source_edge(max_edge);
    run_off_main(move || render_local_thumbnail(&p, edge, Resample::High)).await
}

/// Neither this path nor the `library_thumbnail` fallback has a `Config` in
/// scope, so an omitted `max_edge` resolves to the serde default rather than to
/// the user's `thumbnail_size`. A configured daemon may render a grid tile at a
/// different edge than the fallback does; the tile scales the result to its own
/// box either way.
fn clamp_source_edge(requested: Option<u32>) -> u32 {
    requested
        .unwrap_or(DEFAULT_THUMBNAIL_SIZE)
        .clamp(THUMBNAIL_MIN_EDGE, SOURCE_THUMBNAIL_MAX_EDGE)
}

fn render_local_thumbnail(
    path: &std::path::Path,
    max_edge: u32,
    quality: Resample,
) -> Result<Value, IpcError> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| IpcError::invalid(format!("rejecting path '{}': {e}", path.display())))?;
    let img = superpanels_core::image::load_thumbnail(&canonical, max_edge, quality)
        .map_err(|e| IpcError::Image(e.to_string()))?;
    let bytes =
        superpanels_core::image::encode_png(&img).map_err(|e| IpcError::Image(e.to_string()))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(json!({ "data": encoded, "mime": "image/png" }))
}

#[tauri::command]
pub(crate) async fn library_rescan(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main("library_rescan", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn library_delete(
    path: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "library_delete",
        json!({ "path": path }),
        state.config_path(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn library_tag(
    path: String,
    tag: String,
    on: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "library_tag",
        json!({ "path": path, "tag": tag, "on": on }),
        state.config_path(),
    )
    .await
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

        let value = render_local_thumbnail(&path, DEFAULT_THUMBNAIL_SIZE, Resample::Fast)
            .expect("render local thumbnail");
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
        let err =
            tauri::async_runtime::block_on(source_thumbnail("relative/path.png".to_owned(), None))
                .unwrap_err();
        assert!(matches!(err, IpcError::InvalidArgument(_)));
    }

    #[test]
    fn clamp_source_edge_bounds_a_webview_supplied_max_edge() {
        // The frontend picks the edge, so a hostile or buggy webview must not
        // be able to drive an unbounded decode/encode. Omitting it falls back
        // to the grid-sized default rather than the maximum.
        assert_eq!(clamp_source_edge(None), DEFAULT_THUMBNAIL_SIZE);
        assert_eq!(clamp_source_edge(Some(1536)), 1536);
        assert_eq!(clamp_source_edge(Some(0)), THUMBNAIL_MIN_EDGE);
        assert_eq!(clamp_source_edge(Some(u32::MAX)), SOURCE_THUMBNAIL_MAX_EDGE);
    }

    #[test]
    fn render_local_thumbnail_downscales_to_the_requested_edge() {
        // The canvas asks for a bigger edge than the grid; a source larger
        // than the cap must come back bounded by it, aspect preserved.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.png");
        image::RgbImage::from_pixel(400, 200, image::Rgb([9, 9, 9]))
            .save(&path)
            .expect("write fixture png");

        let value = render_local_thumbnail(&path, 100, Resample::High).expect("render");
        let data = value.get("data").and_then(Value::as_str).expect("data");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .expect("base64 decodes");
        let img = image::load_from_memory(&bytes).expect("decode png");
        assert_eq!((img.width(), img.height()), (100, 50));
    }

    #[test]
    fn library_thumbnail_fallback_only_fires_on_daemon_unreachable() {
        // Confused-deputy guard for `library_thumbnail`'s fallback
        //: the local render must only run when the bridge
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
