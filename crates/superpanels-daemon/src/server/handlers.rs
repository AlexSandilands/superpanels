//! Daemon-side IPC handlers for SPEC §12.4 commands that aren't tied to the
//! apply / slideshow pipelines. Library handlers live in the `library`
//! submodule (`SPEC §17` path-validation + thumbnailing).

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use superpanels_core::config::Config;
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::layout::{BezelConfig, FitMode, compute_crop_specs};
use tokio::sync::Mutex;
use tracing::info;

use crate::state::DaemonState;

mod library;

pub(crate) use library::{cmd_library_list, cmd_library_tag, cmd_library_thumbnail};

pub(super) async fn cmd_list_profiles(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    IpcResponse::success(&guard.config.profiles)
}

pub(super) async fn cmd_save_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let Some(profile_val) = req.params.get("profile") else {
        return IpcResponse::failure("params.profile required");
    };
    let profile: superpanels_core::Profile = match serde_json::from_value(profile_val.clone()) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(format!("profile is malformed: {e}")),
    };

    let mut guard = state.lock().await;
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
    guard.config = new_cfg;
    info!("config saved");
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
    let bezel_h = req
        .params
        .get("bezel_h_mm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let bezel_v = req
        .params
        .get("bezel_v_mm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fit = match req
        .params
        .get("fit")
        .and_then(Value::as_str)
        .unwrap_or("fill")
    {
        "fill" => FitMode::Fill,
        "fit" => FitMode::Fit,
        "stretch" => FitMode::Stretch,
        "center" => FitMode::Center,
        other => return IpcResponse::failure(format!("unknown fit `{other}`")),
    };
    let bezels = match (bezel_mm_to_f32(bezel_h), bezel_mm_to_f32(bezel_v)) {
        (Some(h), Some(v)) => BezelConfig {
            horizontal_mm: h,
            vertical_mm: v,
        },
        _ => return IpcResponse::failure("bezel_*_mm out of range"),
    };

    let roots = {
        let guard = state.lock().await;
        guard.config.library.roots.clone()
    };
    let canonical = match library::canonicalise_inside_roots(Path::new(image), &roots) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(e),
    };

    // Header-only read: microseconds, no decode budget concern (`SPEC §12.3`).
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
    match compute_crop_specs(&monitors, &bezels, fit, dims) {
        Ok(specs) => IpcResponse::success(&specs),
        Err(e) => IpcResponse::failure(e.to_string()),
    }
}

fn bezel_mm_to_f32(v: f64) -> Option<f32> {
    // reason: f64→f32 narrowing is intentional for bezel mm; bound-check first
    // so the cast cannot produce ±inf/NaN.
    #[allow(clippy::cast_possible_truncation)]
    if v.is_finite() && (-1e6..=1e6).contains(&v) {
        Some(v as f32)
    } else {
        None
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
        assert!(resp.result.unwrap().is_array());
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
    async fn preview_crop_rejects_unknown_fit() {
        let state = make_state(Config::default());
        let resp = cmd_preview_crop(
            req(
                "preview_crop",
                json!({"image": "/never/exists.png", "fit": "magic"}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("unknown fit"));
    }

    #[tokio::test]
    async fn preview_crop_rejects_out_of_range_bezel() {
        let state = make_state(Config::default());
        let resp = cmd_preview_crop(
            req(
                "preview_crop",
                json!({"image": "/never/exists.png", "bezel_h_mm": 1e9}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("out of range"));
    }

    #[tokio::test]
    async fn preview_crop_rejects_path_outside_roots() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        let state = make_state(cfg);
        let resp = cmd_preview_crop(
            req(
                "preview_crop",
                json!({"image": "/etc/passwd", "fit": "fill"}),
            ),
            state,
        )
        .await;
        assert!(!resp.is_ok());
    }

    #[cfg(test)]
    fn sample_profile() -> Profile {
        use std::path::PathBuf;
        use superpanels_core::config::{ProfileBody, SpanProfile, SpanSource};
        use superpanels_core::layout::{BezelConfig, FitMode};
        Profile {
            name: "sample".to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Single {
                    path: PathBuf::from("/img.png"),
                },
                fit: FitMode::Fill,
                offset: [0, 0],
            }),
            bezels: BezelConfig {
                horizontal_mm: 0.0,
                vertical_mm: 0.0,
            },
            backend_override: None,
            schedule: None,
        }
    }
}
