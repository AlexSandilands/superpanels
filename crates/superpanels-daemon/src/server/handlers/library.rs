//! Library IPC handlers. Path-validates client-supplied
//! arguments against the configured library roots and keeps
//! responses under the IPC frame cap by paginating + thumbnailing.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};
use superpanels_core::ipc::validate as v;
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::library::{LibraryEntry, LibraryFilter, apply_library_filter};
use tokio::sync::Mutex;
use tracing::warn;

use crate::state::DaemonState;

/// Force a synchronous rescan of every configured root, persist the result
/// into the library DB, and refresh the in-memory cache. Returns the post-
/// rescan entry count so the GUI can surface "scanned N images" feedback.
pub(crate) async fn cmd_library_rescan(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let mut guard = state.lock().await;
    guard.rescan_library();
    let count = guard.library.len();
    IpcResponse::success(json!({ "count": count }))
}

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

    let (roots, edge) = {
        let guard = state.lock().await;
        (
            guard.config.library.roots.clone(),
            guard.config.library.thumbnail_size,
        )
    };

    let canonical = match canonicalise_inside_roots(Path::new(raw_path), &roots) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e),
    };

    // Cache key needs the source mtime so a write to the file invalidates
    // stale bytes the next time we stat it. A failed stat just skips the
    // cache — the render path stays correct.
    let mtime = std::fs::metadata(&canonical)
        .ok()
        .and_then(|m| m.modified().ok());

    if let Some(mtime) = mtime {
        let mut guard = state.lock().await;
        if let Some(cached) = guard.thumbnail_cache.get(&canonical, mtime) {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&cached.bytes);
            return IpcResponse::success(json!({ "data": encoded, "mime": cached.mime }));
        }
    }

    let canonical_for_task = canonical.clone();
    let result =
        tokio::task::spawn_blocking(move || render_thumbnail(&canonical_for_task, edge)).await;
    cache_and_respond(result, state, canonical, mtime).await
}

// reason: clippy's `similar_names` flags `mime` vs `mtime`, but `mime` is the
// established render-path name and renaming would just trade the lint for
// reviewer confusion.
#[allow(clippy::similar_names)]
async fn cache_and_respond(
    result: Result<Result<(Vec<u8>, &'static str), String>, tokio::task::JoinError>,
    state: Arc<Mutex<DaemonState>>,
    canonical: PathBuf,
    mtime: Option<std::time::SystemTime>,
) -> IpcResponse {
    match result {
        Ok(Ok((bytes, mime))) => {
            if let Some(mtime) = mtime {
                let mut guard = state.lock().await;
                guard
                    .thumbnail_cache
                    .put(canonical, mtime, bytes.clone(), mime);
            }
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            IpcResponse::success(json!({ "data": encoded, "mime": mime }))
        }
        Ok(Err(e)) => IpcResponse::failure(e),
        Err(e) => IpcResponse::failure(format!("thumbnail task panicked: {e}")),
    }
}

pub(crate) async fn cmd_library_delete(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(raw_path) = req.params.get("path").and_then(Value::as_str) else {
        return IpcResponse::failure("params.path (string) required");
    };

    let roots = state.lock().await.config.library.roots.clone();
    let canonical = match canonicalise_inside_roots(Path::new(raw_path), &roots) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e),
    };

    let mut guard = state.lock().await;
    let removed_from_db = if let Some(db) = guard.library_db.as_mut() {
        let by_canonical = match db.delete_entry(&canonical) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "library DB delete failed");
                return IpcResponse::failure(format!("library DB delete failed: {e}"));
            }
        };
        if by_canonical {
            true
        } else {
            // Fall back to the raw path so legacy entries indexed before
            // canonicalisation still delete.
            match db.delete_entry(Path::new(raw_path)) {
                Ok(b) => b,
                Err(e) => {
                    warn!(error = %e, "library DB delete failed");
                    return IpcResponse::failure(format!("library DB delete failed: {e}"));
                }
            }
        }
    } else {
        false
    };
    let before = guard.library.len();
    let raw_target = Path::new(raw_path);
    guard
        .library
        .retain(|e| e.path != canonical && e.path != raw_target);
    let removed_from_cache = guard.library.len() < before;

    if !removed_from_db && !removed_from_cache {
        return IpcResponse::failure(format!("path '{raw_path}' not in library"));
    }
    IpcResponse::success(json!({ "path": raw_path }))
}

pub(crate) async fn cmd_library_tag(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(raw_path) = req.params.get("path").and_then(Value::as_str) else {
        return IpcResponse::failure("params.path (string) required");
    };
    let Some(raw_tag) = req.params.get("tag").and_then(Value::as_str) else {
        return IpcResponse::failure("params.tag (string) required");
    };
    let Some(on) = req.params.get("on").and_then(Value::as_bool) else {
        return IpcResponse::failure("params.on (bool) required");
    };
    let tag = match v::validate_tag(raw_tag) {
        Ok(t) => t,
        Err(e) => return IpcResponse::failure(e.0),
    };

    let roots = state.lock().await.config.library.roots.clone();
    let canonical = match canonicalise_inside_roots(Path::new(raw_path), &roots) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e),
    };

    let mut guard = state.lock().await;
    let raw_target = Path::new(raw_path);
    let entry_path = guard
        .library
        .iter()
        .find(|e| e.path == canonical || e.path == raw_target)
        .map(|e| e.path.clone());
    let Some(entry_path) = entry_path else {
        return IpcResponse::failure(format!("path '{raw_path}' not in library"));
    };
    if let Some(db) = guard.library_db.as_mut() {
        if let Err(e) = db.set_tag(&entry_path, &tag, on) {
            warn!(error = %e, "library DB tag write failed");
            return IpcResponse::failure(format!("library DB tag write failed: {e}"));
        }
    }
    if let Some(entry) = guard.library.iter_mut().find(|e| e.path == entry_path) {
        apply_tag(entry, &tag, on);
    }
    IpcResponse::success(json!({ "path": raw_path, "tag": tag, "on": on }))
}

fn apply_tag(entry: &mut LibraryEntry, tag: &str, on: bool) {
    if tag.eq_ignore_ascii_case("favourite") {
        entry.favourite = on;
        return;
    }
    let normalised = tag.trim().to_ascii_lowercase();
    if normalised.is_empty() {
        return;
    }
    let position = entry
        .tags
        .iter()
        .position(|t| t.eq_ignore_ascii_case(&normalised));
    match (on, position) {
        (true, None) => entry.tags.push(normalised),
        (false, Some(idx)) => {
            entry.tags.swap_remove(idx);
        }
        _ => {}
    }
}

/// Canonicalise `requested` and ensure the result lives under one of the
/// configured library roots. Returns the canonical path on
/// success or a user-facing error string on rejection.
///
/// **Fail-deny by construction.** Empty roots reject. A failure to canonicalise
/// `requested` rejects (no symlink-race window — the resolved path is what we
/// compare). A root that itself fails to canonicalise (deleted, EACCES,
/// symlink loop) is *silently skipped* from the allowlist via `is_ok_and`,
/// so a misconfigured or unreadable root reduces permitted paths instead of
/// expanding them. Don't replace `is_ok_and` with `unwrap_or` or `?`-propagation
/// without re-reasoning about that property.
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

fn render_thumbnail(path: &Path, edge: u32) -> Result<(Vec<u8>, &'static str), String> {
    let img = superpanels_core::image::load_thumbnail(
        path,
        edge,
        superpanels_core::image::Resample::Fast,
    )
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
    async fn library_thumbnail_second_call_hits_cache() {
        // First call decodes + resizes + encodes; second call for the same
        // (path, mtime) must satisfy from the cache. We verify by inspecting
        // the cache state directly — driving wallclock-timing assertions in a
        // unit test would be flaky.
        let dir = tempdir().unwrap();
        let img_path = dir.path().join("img.png");
        write_dummy_png(&img_path, 64, 64);

        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);

        let first = cmd_library_thumbnail(
            req(
                "library_thumbnail",
                json!({"path": img_path.to_string_lossy()}),
            ),
            Arc::clone(&state),
        )
        .await;
        assert!(first.is_ok(), "first render failed: {:?}", first.error);
        assert_eq!(state.lock().await.thumbnail_cache.len(), 1);

        let second = cmd_library_thumbnail(
            req(
                "library_thumbnail",
                json!({"path": img_path.to_string_lossy()}),
            ),
            Arc::clone(&state),
        )
        .await;
        assert!(second.is_ok());
        // Both calls return identical base64 — the cache must serve byte-for-byte.
        assert_eq!(first.result, second.result);
        // Still exactly one entry — no double-counting on hit.
        assert_eq!(state.lock().await.thumbnail_cache.len(), 1);
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
        // Path is real and inside roots, but not indexed — must fall through
        // to the "not in library" check.
        let dir = tempdir().unwrap();
        let real = dir.path().join("orphan.png");
        write_dummy_png(&real, 16, 16);
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);
        let resp = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": real.to_string_lossy(), "tag": "x", "on": true}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("not in library"));
    }

    #[tokio::test]
    async fn library_tag_rejects_oversize_tag() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("img.png");
        write_dummy_png(&real, 16, 16);
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let mut s = DaemonState::for_tests(cfg);
        s.library.push(dummy_entry(&real));
        let state = Arc::new(Mutex::new(s));

        let big_tag = "x".repeat(super::v::MAX_TAG_CHARS + 1);
        let resp = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": real.to_string_lossy(), "tag": big_tag, "on": true}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("exceeds"));
    }

    #[tokio::test]
    async fn library_tag_rejects_path_outside_roots() {
        let dir = tempdir().unwrap();
        let other = tempdir().unwrap();
        let foreign = other.path().join("a.png");
        write_dummy_png(&foreign, 16, 16);
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);
        let resp = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": foreign.to_string_lossy(), "tag": "x", "on": true}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("outside"));
    }

    #[tokio::test]
    async fn library_tag_toggles_tag_and_favourite() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("x.png");
        write_dummy_png(&real, 16, 16);
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let mut s = DaemonState::for_tests(cfg);
        s.library.push(dummy_entry(&real));
        let state = Arc::new(Mutex::new(s));

        let _ = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": real.to_string_lossy(), "tag": "blue", "on": true}),
            ),
            Arc::clone(&state),
        )
        .await;
        let _ = cmd_library_tag(
            req(
                "library_tag",
                json!({"path": real.to_string_lossy(), "tag": "favourite", "on": true}),
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
                json!({"path": real.to_string_lossy(), "tag": "blue", "on": false}),
            ),
            Arc::clone(&state),
        )
        .await;
        assert!(state.lock().await.library[0].tags.is_empty());
    }

    /// Drives `apply_tag` directly so we can exercise the no-op match arms
    /// without touching the daemon flow / canonical path resolution.
    fn fresh_entry() -> LibraryEntry {
        dummy_entry(Path::new("/never/used.png"))
    }

    #[test]
    fn apply_tag_on_already_present_tag_is_noop() {
        // (true, Some(_)) — re-asserting an existing tag must not append a
        // duplicate. Earlier code accidentally appended on a case-mismatched
        // tag because the equality predicate was case-sensitive; the case-
        // insensitive lookup is exactly what this arm exercises.
        let mut e = fresh_entry();
        e.tags.push("blue".to_owned());
        apply_tag(&mut e, "blue", true);
        assert_eq!(e.tags, vec!["blue"]);
        // Case-insensitive: "BLUE" must not duplicate "blue" either.
        apply_tag(&mut e, "BLUE", true);
        assert_eq!(e.tags, vec!["blue"]);
    }

    #[test]
    fn apply_tag_off_for_missing_tag_is_noop() {
        // (false, None) — clearing a tag that isn't present is fine.
        let mut e = fresh_entry();
        apply_tag(&mut e, "missing", false);
        assert!(e.tags.is_empty());
        // And it doesn't accidentally clear unrelated tags.
        e.tags.push("red".to_owned());
        apply_tag(&mut e, "blue", false);
        assert_eq!(e.tags, vec!["red"]);
    }

    #[test]
    fn apply_tag_favourite_clears_when_off() {
        // The favourite branch is symmetric: passing on=false must clear the
        // flag. Existing coverage only set it to true.
        let mut e = fresh_entry();
        e.favourite = true;
        apply_tag(&mut e, "favourite", false);
        assert!(!e.favourite);
        // Idempotent — clearing again stays cleared.
        apply_tag(&mut e, "favourite", false);
        assert!(!e.favourite);
        // Case-insensitive on the magic name.
        e.favourite = true;
        apply_tag(&mut e, "FAVOURITE", false);
        assert!(!e.favourite);
    }

    #[tokio::test]
    async fn library_list_aggregates_entries_from_multiple_roots() {
        // `rescan_library` walks each `config.library.roots` entry and
        // concatenates. Pin that with a 2-root fixture so a regression
        // (e.g. "scans only roots[0]") shows up as a missing entry on
        // `library_list`. Mirrors the in-process equivalent in
        // `superpanels-gui/src/commands/in_process.rs`.
        let root_a = tempdir().unwrap();
        let root_b = tempdir().unwrap();
        let img_a = root_a.path().join("a.png");
        let img_b = root_b.path().join("b.png");
        write_dummy_png(&img_a, 8, 8);
        write_dummy_png(&img_b, 8, 8);

        let mut cfg = Config::default();
        cfg.library.roots = vec![root_a.path().to_path_buf(), root_b.path().to_path_buf()];
        let mut s = DaemonState::for_tests(cfg);
        s.rescan_library();
        let state = make_state_from(s);

        let resp = cmd_library_list(req("library_list", json!({})), state).await;
        assert!(resp.is_ok());
        let arr = resp.result.unwrap();
        let arr = arr.as_array().expect("library_list returns array");
        let names: Vec<String> = arr
            .iter()
            .filter_map(|e| {
                e.get("path")
                    .and_then(Value::as_str)
                    .and_then(|p| Path::new(p).file_name()?.to_str())
                    .map(str::to_owned)
            })
            .collect();
        assert!(
            names.contains(&"a.png".to_owned()),
            "missing root_a entry: {names:?}"
        );
        assert!(
            names.contains(&"b.png".to_owned()),
            "missing root_b entry: {names:?}"
        );
    }

    fn make_state_from(s: DaemonState) -> Arc<Mutex<DaemonState>> {
        Arc::new(Mutex::new(s))
    }

    #[test]
    fn apply_tag_ignores_blank_tag_after_trim() {
        // The whitespace-only branch is a silent no-op by design — guards
        // against the IPC layer accidentally letting an empty payload through.
        let mut e = fresh_entry();
        apply_tag(&mut e, "   ", true);
        apply_tag(&mut e, "", true);
        assert!(e.tags.is_empty());
    }
}
