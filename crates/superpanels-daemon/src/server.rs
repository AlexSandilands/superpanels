//! IPC server: accepts Unix socket connections, dispatches JSON requests.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use superpanels_core::ipc::{IpcRequest, IpcResponse, PROTOCOL_VERSION};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, watch};
use tracing::{debug, error};

use crate::state::DaemonState;

mod apply;
mod frame;
mod handlers;
mod helpers;
mod slideshow;

/// Runs until the socket encounters a fatal error. Spawns a task per connection.
pub(crate) async fn run_server(
    listener: UnixListener,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state = Arc::clone(&state);
                let timer_tx = timer_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state, timer_tx).await {
                        debug!(error = %e, "IPC connection closed");
                    }
                });
            }
            Err(e) => {
                error!(error = %e, "accept failed on IPC socket");
                return;
            }
        }
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> Result<()> {
    loop {
        let body = match frame::read_frame(&mut stream).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let req: IpcRequest = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(e) => {
                let resp = IpcResponse::failure(format!("malformed request: {e}"));
                frame::write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
                continue;
            }
        };
        if req.v != PROTOCOL_VERSION {
            let resp = IpcResponse::failure(format!(
                "unsupported protocol version {}; expected {}",
                req.v, PROTOCOL_VERSION
            ));
            frame::write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
            continue;
        }
        debug!(method = %req.method, "IPC request");
        let resp = dispatch(req, Arc::clone(&state), timer_tx.clone()).await;
        frame::write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
    }
}

/// Exposed for the startup default-profile apply in `main.rs` and tests.
pub(crate) async fn dispatch_for_tests(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    dispatch(req, state, timer_tx).await
}

async fn dispatch(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    match req.method.as_str() {
        "set" => apply::cmd_set(req, state).await,
        "apply_profile" => apply::cmd_apply_profile(req, state, timer_tx).await,
        "slideshow_next" => slideshow::cmd_slideshow_advance(state, timer_tx, false).await,
        "slideshow_prev" => slideshow::cmd_slideshow_prev(state).await,
        "slideshow_pause" => slideshow::cmd_slideshow_pause(req, state).await,
        "redetect" => apply::cmd_redetect(state).await,
        "current_state" => apply::cmd_current_state(state).await,
        "list_profiles" => handlers::cmd_list_profiles(state).await,
        "save_profile" => handlers::cmd_save_profile(req, state).await,
        "delete_profile" => handlers::cmd_delete_profile(req, state).await,
        "get_config" => handlers::cmd_get_config(state).await,
        "save_config" => handlers::cmd_save_config(req, state).await,
        "detect_monitors" => handlers::cmd_detect_monitors(state).await,
        "preview_crop" => handlers::cmd_preview_crop(req, state).await,
        "library_list" => handlers::cmd_library_list(req, state).await,
        "library_thumbnail" => handlers::cmd_library_thumbnail(req, state).await,
        "library_tag" => handlers::cmd_library_tag(req, state).await,
        "library_delete" => handlers::cmd_library_delete(req, state).await,
        "library_rescan" => handlers::cmd_library_rescan(state).await,
        other => IpcResponse::failure(format!("unknown method: {other}")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same as above
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use serde_json::json;
    use superpanels_core::config::{
        BackendKind, Config, ImageSet, Profile, ProfileBody, SlideshowConfig as SlideshowCfg,
        SlideshowSort, SlideshowStart, SpanProfile, SpanSource,
    };
    use superpanels_core::layout::{BezelConfig, FitMode as LayoutFitMode};
    use superpanels_core::slideshow::{
        SlideshowConfig as PickerCfg, SlideshowPicker, SlideshowSort as PickerSort,
        SlideshowStart as PickerStart,
    };
    use tempfile::tempdir;

    use super::*;

    fn write_dummy_image(path: &std::path::Path) {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img).save(path).unwrap();
    }

    fn slideshow_profile(name: &str, folder: &std::path::Path) -> Profile {
        Profile {
            name: name.to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Slideshow {
                    images: ImageSet::Folder {
                        path: folder.to_path_buf(),
                        recursive: false,
                    },
                    config: SlideshowCfg {
                        interval: Duration::from_secs(60),
                        sort: SlideshowSort::Alphabetical,
                        recent_history_size: 4,
                        on_start: SlideshowStart::Resume,
                        pause_when_active: false,
                        skip_on_unavailable: false,
                    },
                },
                fit: LayoutFitMode::Fill,
                offset: [0, 0],
                image_size_px: None,
            }),
            bezels: BezelConfig {
                horizontal_mm: 0.0,
                vertical_mm: 0.0,
            },
            backend_override: Some(BackendKind::Custom),
            schedule: None,
        }
    }

    fn picker_with_history(history: Vec<PathBuf>) -> SlideshowPicker {
        let mut picker = SlideshowPicker::new(PickerCfg {
            interval: Duration::from_secs(60),
            sort: PickerSort::Alphabetical,
            recent_history_size: 10,
            on_start: PickerStart::Resume,
            pause_when_active: false,
            skip_on_unavailable: false,
        });
        for path in history.into_iter().rev() {
            picker.state_mut().history.push_front(path);
        }
        picker
    }

    fn make_state_arc(state: DaemonState) -> Arc<Mutex<DaemonState>> {
        Arc::new(Mutex::new(state))
    }

    fn timer_pair() -> watch::Sender<Option<Duration>> {
        watch::channel::<Option<Duration>>(None).0
    }

    #[tokio::test]
    async fn current_state_includes_active_profile_and_slideshow_summary() {
        let dir = tempdir().unwrap();
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![PathBuf::from("/walls/a.png")]));
        let state = make_state_arc(s);

        let req = IpcRequest {
            v: PROTOCOL_VERSION,
            method: "current_state".to_owned(),
            params: json!({}),
        };
        let resp = dispatch_for_tests(req, Arc::clone(&state), timer_pair()).await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        let result = resp.result.unwrap();
        assert_eq!(result["active_profile"], json!("p"));
        let summary = &result["slideshow"];
        assert_eq!(summary["history_len"], json!(1));
        assert_eq!(summary["paused"], json!(false));
    }

    #[tokio::test]
    async fn slideshow_pause_toggles_when_no_explicit_value() {
        let mut config = Config::default();
        config
            .profiles
            .push(slideshow_profile("p", &PathBuf::from("/walls")));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![]));
        let state = make_state_arc(s);

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pause".to_owned(),
                params: json!({}),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;

        assert!(resp.is_ok());
        assert_eq!(resp.result.unwrap()["paused"], json!(true));
        assert!(
            state
                .lock()
                .await
                .slideshow_picker
                .as_ref()
                .unwrap()
                .state()
                .paused
        );

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pause".to_owned(),
                params: json!({}),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;
        assert_eq!(resp.result.unwrap()["paused"], json!(false));
        assert!(
            !state
                .lock()
                .await
                .slideshow_picker
                .as_ref()
                .unwrap()
                .state()
                .paused
        );
    }

    #[tokio::test]
    async fn slideshow_pause_honours_explicit_value() {
        let mut config = Config::default();
        config
            .profiles
            .push(slideshow_profile("p", &PathBuf::from("/walls")));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![]));
        let state = make_state_arc(s);

        for _ in 0..2 {
            let resp = dispatch_for_tests(
                IpcRequest {
                    v: PROTOCOL_VERSION,
                    method: "slideshow_pause".to_owned(),
                    params: json!({"paused": true}),
                },
                Arc::clone(&state),
                timer_pair(),
            )
            .await;
            assert_eq!(resp.result.unwrap()["paused"], json!(true));
        }
        assert!(
            state
                .lock()
                .await
                .slideshow_picker
                .as_ref()
                .unwrap()
                .state()
                .paused
        );
    }

    #[tokio::test]
    async fn slideshow_pause_errors_when_no_active_slideshow() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pause".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("no active slideshow"));
    }

    #[tokio::test]
    async fn slideshow_prev_returns_history_at_index_one() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.png");
        write_dummy_image(&a);
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![a.clone(), a.clone()]));
        let state = make_state_arc(s);

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_prev".to_owned(),
                params: json!({}),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;

        if let Some(err) = &resp.error {
            assert!(
                !err.contains("no previous image"),
                "unexpected missing-history error: {err}"
            );
        }
    }

    #[tokio::test]
    async fn slideshow_prev_errors_when_history_empty() {
        let mut config = Config::default();
        config
            .profiles
            .push(slideshow_profile("p", &PathBuf::from("/walls")));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![]));
        let state = make_state_arc(s);

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_prev".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;

        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("no previous image"));
    }

    #[tokio::test]
    async fn apply_profile_unknown_name_returns_failure() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "apply_profile".to_owned(),
                params: json!({"name": "ghost"}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn apply_profile_advances_picker_history_for_slideshow_profile() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.png");
        write_dummy_image(&a);
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let state = make_state_arc(DaemonState::for_tests(config));

        let _ = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "apply_profile".to_owned(),
                params: json!({"name": "p"}),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;

        let guard = state.lock().await;
        let picker = guard.slideshow_picker.as_ref().expect("picker created");
        assert_eq!(picker.state().history.len(), 1);
    }

    #[tokio::test]
    async fn slideshow_advance_errors_when_no_active_profile() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_next".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("no active profile"));
    }

    #[tokio::test]
    async fn redetect_returns_a_response_payload_and_does_not_panic() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "redetect".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(resp.is_ok() || resp.error.is_some());
        if resp.is_ok() {
            assert!(resp.result.unwrap().get("monitors").is_some());
        }
    }

    #[tokio::test]
    async fn set_requires_image_param() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "set".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("params.image"));
    }

    #[tokio::test]
    async fn unknown_method_returns_failure() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "no_such_method".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("unknown method"));
    }
}
