//! Library IPC handlers (`SPEC §12.4`). Path-validates client-supplied
//! arguments against the configured library roots (`SPEC §17`) and keeps
//! responses under the IPC frame cap by paginating + thumbnailing.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::library::{LibraryEntry, LibraryFilter, apply_library_filter, persist_index};
use tokio::sync::Mutex;
use tracing::warn;

use crate::state::DaemonState;

const THUMBNAIL_MAX_EDGE: u32 = 320;

pub(crate) async fn cmd_library_list(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    // reason: filter is optional; an absent or malformed payload falls back
    // to the default filter (paginated to DEFAULT_LIBRARY_PAGE entries).
    let filter: LibraryFilter = req
        .params
        .get("filter")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let page: Vec<LibraryEntry> = {
        let guard = state.lock().await;
        apply_library_filter(&guard.library, &filter)
    };
    IpcResponse::success(&page)
}

pub(crate) async fn cmd_library_thumbnail(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(raw_path) = req.params.get("path").and_then(Value::as_str) else {
        return IpcResponse::failure("params.path (string) required");
    };

    let roots = {
        let guard = state.lock().await;
        guard.config.library.roots.clone()
    };

    let canonical = match canonicalise_inside_roots(Path::new(raw_path), &roots) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e),
    };

    let result = tokio::task::spawn_blocking(move || render_thumbnail(&canonical)).await;
    match result {
        Ok(Ok((bytes, mime))) => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            IpcResponse::success(json!({ "data": encoded, "mime": mime }))
        }
        Ok(Err(e)) => IpcResponse::failure(e),
        Err(e) => IpcResponse::failure(format!("thumbnail task panicked: {e}")),
    }
}

pub(crate) async fn cmd_library_tag(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(path) = req.params.get("path").and_then(Value::as_str) else {
        return IpcResponse::failure("params.path (string) required");
    };
    let Some(tag) = req.params.get("tag").and_then(Value::as_str) else {
        return IpcResponse::failure("params.tag (string) required");
    };
    let Some(on) = req.params.get("on").and_then(Value::as_bool) else {
        return IpcResponse::failure("params.on (bool) required");
    };

    let mut guard = state.lock().await;
    let target = Path::new(path);
    let Some(entry) = guard.library.iter_mut().find(|e| e.path == target) else {
        return IpcResponse::failure(format!("path '{path}' not in library"));
    };
    apply_tag(entry, tag, on);

    if let Some(state_dir) = DaemonState::state_dir() {
        let index_path = state_dir.join("library-index.json");
        if let Err(e) = persist_index(&guard.library, &index_path) {
            warn!(error = %e, "failed to persist library index after tag update");
        }
    }
    IpcResponse::success(json!({ "path": path, "tag": tag, "on": on }))
}

fn apply_tag(entry: &mut LibraryEntry, tag: &str, on: bool) {
    if tag == "favourite" {
        entry.favourite = on;
        return;
    }
    let owned = tag.to_owned();
    let position = entry.tags.iter().position(|t| t == &owned);
    match (on, position) {
        (true, None) => entry.tags.push(owned),
        (false, Some(idx)) => {
            entry.tags.swap_remove(idx);
        }
        _ => {}
    }
}

/// Canonicalise `requested` and ensure the result lives under one of the
/// configured library roots (`SPEC §17`). Returns the canonical path on
/// success or a user-facing error string on rejection.
pub(crate) fn canonicalise_inside_roots(
    requested: &Path,
    roots: &[PathBuf],
) -> Result<PathBuf, String> {
    if roots.is_empty() {
        return Err("library has no configured roots".to_owned());
    }
    let canonical = std::fs::canonicalize(requested)
        .map_err(|e| format!("rejecting path '{}': {e}", requested.display()))?;
    let allowed = roots
        .iter()
        .any(|root| std::fs::canonicalize(root).is_ok_and(|c| canonical.starts_with(&c)));
    if !allowed {
        return Err(format!(
            "path '{}' is outside the configured library roots",
            requested.display()
        ));
    }
    Ok(canonical)
}

fn render_thumbnail(path: &Path) -> Result<(Vec<u8>, &'static str), String> {
    let img = superpanels_core::image::load_thumbnail(path, THUMBNAIL_MAX_EDGE)
        .map_err(|e| e.to_string())?;
    let bytes = superpanels_core::image::encode_png(&img).map_err(|e| e.to_string())?;
    Ok((bytes, "image/png"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same
mod tests {
    use std::path::PathBuf;
    use std::time::SystemTime;

    use serde_json::json;
    use superpanels_core::config::Config;
    use superpanels_core::ipc::PROTOCOL_VERSION;
    use tempfile::tempdir;

    use super::*;

    fn req(method: &str, params: Value) -> IpcRequest {
        IpcRequest {
            v: PROTOCOL_VERSION,
            method: method.to_owned(),
            params,
        }
    }

    fn dummy_entry(path: &Path) -> LibraryEntry {
        LibraryEntry {
            path: path.to_path_buf(),
            resolution: (1, 1),
            aspect_ratio: 1.0,
            file_size: 0,
            modified: SystemTime::UNIX_EPOCH,
            tags: Vec::new(),
            favourite: false,
            last_shown: None,
            show_count: 0,
        }
    }

    fn make_state(config: Config) -> Arc<Mutex<DaemonState>> {
        Arc::new(Mutex::new(DaemonState::for_tests(config)))
    }

    fn write_dummy_png(path: &Path, w: u32, h: u32) {
        let buf = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(buf).save(path).unwrap();
    }

    #[tokio::test]
    async fn library_list_returns_paginated_entries() {
        // Arrange — 250 entries; default page = 200.
        let mut s = DaemonState::for_tests(Config::default());
        for i in 0..250 {
            let p = PathBuf::from(format!("/walls/{i}.png"));
            s.library.push(dummy_entry(&p));
        }
        let state = Arc::new(Mutex::new(s));

        // Act
        let resp = cmd_library_list(req("library_list", json!({})), state).await;

        // Assert — first page has 200 entries.
        assert!(resp.is_ok());
        let arr = resp.result.unwrap().as_array().unwrap().len();
        assert_eq!(arr, 200);
    }

    #[tokio::test]
    async fn library_list_honours_offset_and_limit() {
        let mut s = DaemonState::for_tests(Config::default());
        for i in 0..50 {
            s.library
                .push(dummy_entry(&PathBuf::from(format!("/w/{i}.png"))));
        }
        let state = Arc::new(Mutex::new(s));

        let resp = cmd_library_list(
            req(
                "library_list",
                json!({"filter": {"offset": 10, "limit": 5}}),
            ),
            state,
        )
        .await;

        assert!(resp.is_ok());
        let v = resp.result.unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 5);
    }

    #[tokio::test]
    async fn library_thumbnail_rejects_path_outside_roots() {
        // Arrange — root is some empty tempdir; client requests a different path.
        let dir = tempdir().unwrap();
        let other = tempdir().unwrap();
        let foreign = other.path().join("a.png");
        write_dummy_png(&foreign, 16, 16);

        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);

        // Act
        let resp = cmd_library_thumbnail(
            req(
                "library_thumbnail",
                json!({"path": foreign.to_string_lossy()}),
            ),
            state,
        )
        .await;

        // Assert
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("outside"));
    }

    #[tokio::test]
    async fn library_thumbnail_rejects_when_no_roots_configured() {
        let cfg = Config::default(); // roots is empty
        let state = make_state(cfg);
        let resp = cmd_library_thumbnail(
            req("library_thumbnail", json!({"path": "/etc/passwd"})),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("no configured roots"));
    }

    #[tokio::test]
    async fn library_thumbnail_returns_base64_png_for_valid_path() {
        let dir = tempdir().unwrap();
        let img_path = dir.path().join("img.png");
        write_dummy_png(&img_path, 64, 64);

        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);

        let resp = cmd_library_thumbnail(
            req(
                "library_thumbnail",
                json!({"path": img_path.to_string_lossy()}),
            ),
            state,
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        let v = resp.result.unwrap();
        assert_eq!(v["mime"], json!("image/png"));
        let data = v["data"].as_str().unwrap();
        // Reasonable PNG of a 64x64 image is well under 1 MiB.
        assert!(!data.is_empty());
        assert!(data.len() < 1024 * 1024);
    }

    #[tokio::test]
    async fn library_thumbnail_requires_path_param() {
        let state = make_state(Config::default());
        let resp = cmd_library_thumbnail(req("library_thumbnail", json!({})), state).await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("params.path"));
    }

    #[tokio::test]
    async fn library_tag_unknown_path_fails() {
        let state = make_state(Config::default());
        let resp = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": "/no/such.png", "tag": "x", "on": true}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("not in library"));
    }

    #[tokio::test]
    async fn library_tag_toggles_tag_and_favourite() {
        let mut s = DaemonState::for_tests(Config::default());
        let path = PathBuf::from("/walls/x.png");
        s.library.push(dummy_entry(&path));
        let state = Arc::new(Mutex::new(s));

        let _ = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": "/walls/x.png", "tag": "blue", "on": true}),
            ),
            Arc::clone(&state),
        )
        .await;
        let _ = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": "/walls/x.png", "tag": "favourite", "on": true}),
            ),
            Arc::clone(&state),
        )
        .await;

        let guard = state.lock().await;
        let entry = &guard.library[0];
        assert!(entry.tags.contains(&"blue".to_owned()));
        assert!(entry.favourite);

        drop(guard);

        let _ = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": "/walls/x.png", "tag": "blue", "on": false}),
            ),
            Arc::clone(&state),
        )
        .await;
        assert!(state.lock().await.library[0].tags.is_empty());
    }
}
