//! Daemon-side IPC handlers for SPEC §12.4 commands that aren't tied to the
//! apply / slideshow pipelines. Library handlers live in the `library`
//! submodule (`SPEC §17` path-validation + thumbnailing).

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use superpanels_core::config::{Config, write_monitor_block};
use superpanels_core::ipc::validate as v;
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::layout::{
    ImageRectMm, compute_crop_specs, cover_image_rect_mm, synthesise_placements,
};
use tokio::sync::Mutex;
use tracing::info;

use crate::state::DaemonState;

mod library;

pub(crate) use library::{
    cmd_library_delete, cmd_library_list, cmd_library_rescan, cmd_library_tag,
    cmd_library_thumbnail,
};

pub(super) async fn cmd_list_profiles(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    let validity: Vec<serde_json::Value> = guard
        .config
        .profiles
        .iter()
        .map(|p| {
            let v = superpanels_core::ProfileValidity::evaluate(p, &guard.monitors);
            json!({"profile": p.name, "validity": v})
        })
        .collect();
    IpcResponse::success(json!({
        "profiles": &guard.config.profiles,
        "validity": validity,
    }))
}

pub(super) async fn cmd_duplicate_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(name) = req.params.get("name").and_then(Value::as_str) else {
        return IpcResponse::failure("params.name (string) required");
    };
    let Some(new_name) = req.params.get("new_name").and_then(Value::as_str) else {
        return IpcResponse::failure("params.new_name (string) required");
    };
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    let Some(source) = guard
        .config
        .profiles
        .iter()
        .find(|p| p.name == name)
        .cloned()
    else {
        return IpcResponse::failure(format!("profile '{name}' not found"));
    };
    if guard.config.profiles.iter().any(|p| p.name == new_name) {
        return IpcResponse::failure(format!("profile '{new_name}' already exists"));
    }
    let now = superpanels_core::config::now_timestamp();
    let mut copy = source;
    new_name.clone_into(&mut copy.name);
    copy.created_at = now;
    copy.updated_at = now;
    copy.last_applied_at = None;
    guard.config.profiles.push(copy);
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    info!(name = new_name, "profile duplicated");
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_rename_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(old) = req.params.get("name").and_then(Value::as_str) else {
        return IpcResponse::failure("params.name (string) required");
    };
    let Some(new_name) = req.params.get("new_name").and_then(Value::as_str) else {
        return IpcResponse::failure("params.new_name (string) required");
    };
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    if guard.config.profiles.iter().any(|p| p.name == new_name) {
        return IpcResponse::failure(format!("profile '{new_name}' already exists"));
    }
    let Some(target) = guard.config.profiles.iter_mut().find(|p| p.name == old) else {
        return IpcResponse::failure(format!("profile '{old}' not found"));
    };
    new_name.clone_into(&mut target.name);
    target.touch();
    let active_renamed = guard.active_profile.as_deref() == Some(old);
    if active_renamed {
        guard.active_profile = Some(new_name.to_owned());
    }
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_update_profile_monitor_state(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(name) = req.params.get("profile").and_then(Value::as_str) else {
        return IpcResponse::failure("params.profile (string) required");
    };
    let Some(stable_id) = req.params.get("stable_id").and_then(Value::as_str) else {
        return IpcResponse::failure("params.stable_id (string) required");
    };
    let placement: superpanels_core::MonitorPlacement = match req
        .params
        .get("placement")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
    {
        Some(p) => p,
        None => return IpcResponse::failure("params.placement (MonitorPlacement) required"),
    };
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    let Some(profile) = guard.config.profiles.iter_mut().find(|p| p.name == name) else {
        return IpcResponse::failure(format!("profile '{name}' not found"));
    };
    profile
        .monitor_state
        .insert(stable_id.to_owned(), placement);
    profile.touch();
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_update_profile_image_transform(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(name) = req.params.get("profile").and_then(Value::as_str) else {
        return IpcResponse::failure("params.profile (string) required");
    };
    let image_rect_mm: Option<ImageRectMm> = req
        .params
        .get("image_rect_mm")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    let Some(profile) = guard.config.profiles.iter_mut().find(|p| p.name == name) else {
        return IpcResponse::failure(format!("profile '{name}' not found"));
    };
    if let superpanels_core::config::ProfileBody::Span(span) = &mut profile.body {
        if let Some(rect) = image_rect_mm {
            span.image_rect_mm = rect;
        }
    }
    profile.touch();
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_update_profile_source(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(name) = req.params.get("profile").and_then(Value::as_str) else {
        return IpcResponse::failure("params.profile (string) required");
    };
    let source: superpanels_core::config::SpanSource = match req
        .params
        .get("source")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
    {
        Some(s) => s,
        None => return IpcResponse::failure("params.source (SpanSource) required"),
    };
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    let Some(profile) = guard.config.profiles.iter_mut().find(|p| p.name == name) else {
        return IpcResponse::failure(format!("profile '{name}' not found"));
    };
    if let superpanels_core::config::ProfileBody::Span(span) = &mut profile.body {
        span.source = source;
    } else {
        return IpcResponse::failure("source updates require a Span profile");
    }
    profile.touch();
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_list_schedules(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    IpcResponse::success(json!({
        "schedules": &guard.config.schedules,
        "paused": guard.config.schedules_paused,
    }))
}

pub(super) async fn cmd_save_schedules(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let schedules: Vec<superpanels_core::Schedule> = match req
        .params
        .get("schedules")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
    {
        Some(s) => s,
        None => return IpcResponse::failure("params.schedules (array) required"),
    };
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    for s in &schedules {
        if let Err(e) = superpanels_core::schedule::validate_trigger(&s.trigger) {
            return IpcResponse::failure(e.to_string());
        }
    }
    guard.config.schedules = schedules;
    if let Some((a, b)) = crate::schedule::detect_same_minute_collision(&guard.config) {
        return IpcResponse::failure(format!(
            "schedules collide: rules {a} and {b} fire at the same minute"
        ));
    }
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_set_schedules_paused(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(paused) = req.params.get("paused").and_then(Value::as_bool) else {
        return IpcResponse::failure("params.paused (bool) required");
    };
    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    guard.config.schedules_paused = paused;
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    IpcResponse::success(json!({"paused": paused}))
}

pub(super) async fn cmd_save_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(profile_val) = req.params.get("profile") else {
        return IpcResponse::failure("params.profile required");
    };
    let mut profile: superpanels_core::Profile = match serde_json::from_value(profile_val.clone()) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(format!("profile is malformed: {e}")),
    };
    let recompute_topology = req
        .params
        .get("recompute_topology")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut guard = state.lock().await;
    // Empty topology is always a placeholder (the UI's "new profile" path
    // ships `""`); otherwise the caller must explicitly opt in via
    // `recompute_topology` so we don't silently rewrite a fingerprint the
    // client meant to preserve.
    if recompute_topology || profile.topology.0.is_empty() {
        profile.topology = superpanels_core::TopologyFingerprint::from_monitors(&guard.monitors);
    }
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    if let Some(existing) = guard
        .config
        .profiles
        .iter_mut()
        .find(|p| p.name == profile.name)
    {
        *existing = profile.clone();
    } else {
        guard.config.profiles.push(profile.clone());
    }
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    info!(name = %profile.name, "profile saved");
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_delete_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(name) = req.params.get("name").and_then(Value::as_str) else {
        return IpcResponse::failure("params.name (string) required");
    };

    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    let before = guard.config.profiles.len();
    guard.config.profiles.retain(|p| p.name != name);
    if guard.config.profiles.len() == before {
        return IpcResponse::failure(format!("profile '{name}' not found"));
    }
    if let Err(e) = guard.config.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    info!(name, "profile deleted");
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_get_config(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    IpcResponse::success(&guard.config)
}

pub(super) async fn cmd_save_config(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(cfg_val) = req.params.get("config") else {
        return IpcResponse::failure("params.config required");
    };
    let new_cfg: Config = match serde_json::from_value(cfg_val.clone()) {
        Ok(c) => c,
        Err(e) => return IpcResponse::failure(format!("config is malformed: {e}")),
    };

    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    if let Err(e) = new_cfg.save_to(&path) {
        return IpcResponse::failure(e.to_string());
    }
    let roots_changed = guard.config.library.roots != new_cfg.library.roots;
    guard.config = new_cfg;
    if roots_changed {
        guard.refresh_watcher();
    }
    info!("config saved");
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_set_monitor_physical_size(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let identifier = match v::parse_monitor_identifier(&req.params) {
        Ok(id) => id,
        Err(e) => return IpcResponse::failure(e.0),
    };
    let physical_mm = match v::parse_physical_mm(&req.params) {
        Ok(mm) => mm,
        Err(e) => return IpcResponse::failure(e.0),
    };

    let mut guard = state.lock().await;
    let path = match guard.config_save_path() {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };
    if let Err(e) = write_monitor_block(&path, &identifier, physical_mm) {
        return IpcResponse::failure(e.to_string());
    }

    // Re-read so the in-memory config reflects what's on disk, then re-merge
    // into the detected monitors so subsequent `detect_monitors` reflects the
    // new physical size without forcing a rescan.
    match Config::load_from(&path) {
        Ok(cfg) => {
            cfg.merge_into_monitors(&mut guard.monitors);
            guard.config = cfg;
        }
        Err(e) => return IpcResponse::failure(e.to_string()),
    }

    info!("monitor physical size updated");
    IpcResponse::success(json!({}))
}

pub(super) async fn cmd_detect_monitors(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    IpcResponse::success(&guard.monitors)
}

pub(super) async fn cmd_preview_crop(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(image) = req.params.get("image").and_then(Value::as_str) else {
        return IpcResponse::failure("params.image (string) required");
    };
    let image_rect_mm: Option<ImageRectMm> = req
        .params
        .get("image_rect_mm")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let roots = {
        let guard = state.lock().await;
        guard.config.library.roots.clone()
    };
    let canonical = match library::canonicalise_inside_roots(Path::new(image), &roots) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e),
    };

    let dims = match tokio::task::spawn_blocking(move || {
        superpanels_core::image::read_dimensions(&canonical)
    })
    .await
    {
        Ok(Ok(d)) => d,
        Ok(Err(e)) => return IpcResponse::failure(e.to_string()),
        Err(e) => return IpcResponse::failure(format!("dimension task panicked: {e}")),
    };

    let monitors = state.lock().await.monitors.clone();
    let placements = synthesise_placements(&monitors);
    let rect = image_rect_mm.unwrap_or_else(|| cover_image_rect_mm(&monitors, dims));
    match compute_crop_specs(&monitors, &placements, dims, rect) {
        Ok(specs) => IpcResponse::success(&specs),
        Err(e) => IpcResponse::failure(e.to_string()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use superpanels_core::config::{Config, Profile};
    use superpanels_core::ipc::PROTOCOL_VERSION;
    use tempfile::tempdir;

    use super::*;

    fn make_state(config: Config) -> Arc<Mutex<DaemonState>> {
        Arc::new(Mutex::new(DaemonState::for_tests(config)))
    }

    fn make_state_with_path(config: Config, path: PathBuf) -> Arc<Mutex<DaemonState>> {
        Arc::new(Mutex::new(DaemonState::for_tests_with_path(config, path)))
    }

    fn req(method: &str, params: Value) -> IpcRequest {
        IpcRequest {
            v: PROTOCOL_VERSION,
            method: method.to_owned(),
            params,
        }
    }

    #[tokio::test]
    async fn list_profiles_returns_profiles_array() {
        let state = make_state(Config::default());
        let resp = cmd_list_profiles(state).await;
        assert!(resp.is_ok());
        let v = resp.result.unwrap();
        assert!(v.get("profiles").is_some());
        assert!(v["profiles"].is_array());
    }

    #[tokio::test]
    async fn save_profile_rejects_malformed_payload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path);
        let resp = cmd_save_profile(
            req("save_profile", json!({"profile": "not-an-object"})),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("malformed"));
    }

    #[tokio::test]
    async fn save_profile_upserts_to_state_and_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path.clone());
        let profile = Profile {
            name: "added".to_owned(),
            ..sample_profile()
        };

        let resp = cmd_save_profile(
            req(
                "save_profile",
                json!({"profile": serde_json::to_value(&profile).unwrap()}),
            ),
            Arc::clone(&state),
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert!(
            state
                .lock()
                .await
                .config
                .profiles
                .iter()
                .any(|p| p.name == "added")
        );
        let on_disk = Config::load_from(&path).unwrap();
        assert!(on_disk.profiles.iter().any(|p| p.name == "added"));
    }

    #[tokio::test]
    async fn delete_profile_missing_returns_failure() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path);
        let resp = cmd_delete_profile(req("delete_profile", json!({"name": "ghost"})), state).await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn delete_profile_removes_from_state_and_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.profiles.push(Profile {
            name: "victim".to_owned(),
            ..sample_profile()
        });
        cfg.save_to(&path).unwrap();
        let state = make_state_with_path(cfg, path.clone());

        let resp = cmd_delete_profile(
            req("delete_profile", json!({"name": "victim"})),
            Arc::clone(&state),
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert!(
            state
                .lock()
                .await
                .config
                .profiles
                .iter()
                .all(|p| p.name != "victim")
        );
        let on_disk = Config::load_from(&path).unwrap();
        assert!(on_disk.profiles.iter().all(|p| p.name != "victim"));
    }

    #[tokio::test]
    async fn save_config_replaces_in_memory_and_on_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path.clone());

        let mut new_cfg = Config::default();
        new_cfg.profiles.push(Profile {
            name: "fromsave".to_owned(),
            ..sample_profile()
        });
        let resp = cmd_save_config(
            req(
                "save_config",
                json!({"config": serde_json::to_value(&new_cfg).unwrap()}),
            ),
            Arc::clone(&state),
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert!(
            state
                .lock()
                .await
                .config
                .profiles
                .iter()
                .any(|p| p.name == "fromsave")
        );
        let on_disk = Config::load_from(&path).unwrap();
        assert!(on_disk.profiles.iter().any(|p| p.name == "fromsave"));
    }

    #[tokio::test]
    async fn save_config_rejects_malformed_payload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path);
        let resp =
            cmd_save_config(req("save_config", json!({"config": "not-a-config"})), state).await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("malformed"));
    }

    #[tokio::test]
    async fn detect_monitors_returns_state_snapshot() {
        let state = make_state(Config::default());
        let resp = cmd_detect_monitors(state).await;
        assert!(resp.is_ok());
        assert!(resp.result.unwrap().is_array());
    }

    #[tokio::test]
    async fn get_config_returns_current_config() {
        let state = make_state(Config::default());
        let resp = cmd_get_config(state).await;
        assert!(resp.is_ok());
        let v = resp.result.unwrap();
        assert!(v.get("general").is_some());
        assert!(v.get("backend").is_some());
    }

    #[tokio::test]
    async fn preview_crop_rejects_path_outside_roots() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);
        let resp =
            cmd_preview_crop(req("preview_crop", json!({"image": "/etc/passwd"})), state).await;
        assert!(!resp.is_ok());
    }

    #[tokio::test]
    async fn set_monitor_physical_size_rejects_above_cap() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path);
        let resp = cmd_set_monitor_physical_size(
            req(
                "set_monitor_physical_size",
                json!({
                    "stable_id": "abc",
                    "physical_mm": [1.0e30, 100.0],
                }),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("must be in (0,"));
    }

    #[tokio::test]
    async fn set_monitor_physical_size_rejects_oversize_stable_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path);
        let big_id = "x".repeat(super::v::MAX_MONITOR_ID_CHARS + 1);
        let resp = cmd_set_monitor_physical_size(
            req(
                "set_monitor_physical_size",
                json!({
                    "stable_id": big_id,
                    "physical_mm": [100.0, 100.0],
                }),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("exceeds"));
    }

    #[tokio::test]
    async fn set_monitor_physical_size_rejects_control_chars_in_name() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let state = make_state_with_path(Config::default(), path);
        let resp = cmd_set_monitor_physical_size(
            req(
                "set_monitor_physical_size",
                json!({
                    "name": "DP-1\nname=injected",
                    "physical_mm": [100.0, 100.0],
                }),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("control"));
    }

    #[tokio::test]
    async fn preview_crop_malformed_image_rect_falls_back_to_cover_fit() {
        // Malformed `image_rect_mm` should not surface as a parser failure;
        // the handler silently falls through to a cover-fit rect. The
        // path-outside-roots check below fires first either way — what we
        // pin is that the error message doesn't mention `image_rect_mm`.
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);

        for malformed in [
            json!({"image": "/etc/passwd", "image_rect_mm": "junk"}),
            json!({"image": "/etc/passwd", "image_rect_mm": [1, 2, 3]}),
            json!({"image": "/etc/passwd", "image_rect_mm": null}),
        ] {
            let resp = cmd_preview_crop(req("preview_crop", malformed), Arc::clone(&state)).await;
            assert!(!resp.is_ok());
            let err = resp.error.unwrap_or_default();
            assert!(
                !err.contains("image_rect_mm"),
                "expected silent fall-through to cover-fit but got: {err}"
            );
        }
    }

    #[cfg(test)]
    fn sample_profile() -> Profile {
        use std::collections::HashMap;
        use std::path::PathBuf;
        use superpanels_core::config::{ProfileBody, SpanProfile, SpanSource};
        use superpanels_core::layout::ImageRectMm;
        use superpanels_core::{ProfileColour, TopologyFingerprint};
        let now = superpanels_core::config::now_timestamp();
        Profile {
            name: "sample".to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Single {
                    path: PathBuf::from("/img.png"),
                },
                image_rect_mm: ImageRectMm::default(),
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            colour: ProfileColour::default(),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: None,
        }
    }
}
