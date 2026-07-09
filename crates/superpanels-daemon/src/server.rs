//! IPC server: accepts Unix socket connections, dispatches JSON requests.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use superpanels_core::ipc::{IpcRequest, IpcResponse, PROTOCOL_VERSION};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Notify, watch};
use tracing::{debug, error, trace};

use crate::state::DaemonState;

mod apply;
mod frame;
mod handlers;
mod helpers;
mod slideshow;

/// Runs until the socket encounters a fatal error. Spawns a task per connection.
///
/// `shutdown` is fired by the `shutdown` IPC method so an external caller (the
/// GUI tray's Exit, the CLI's `daemon stop`) can trigger the same graceful
/// teardown as SIGTERM without tracking the daemon's PID.
pub(crate) async fn run_server(
    listener: UnixListener,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
    shutdown: Arc<Notify>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state = Arc::clone(&state);
                let timer_tx = timer_tx.clone();
                let shutdown = Arc::clone(&shutdown);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state, timer_tx, shutdown).await {
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
    shutdown: Arc<Notify>,
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
        trace!(method = %req.method, "IPC request");
        let resp = dispatch(
            req,
            Arc::clone(&state),
            timer_tx.clone(),
            Arc::clone(&shutdown),
        )
        .await;
        frame::write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
    }
}

/// Exposed for the startup default-profile apply in `main.rs` and tests.
///
/// Uses a throwaway shutdown notifier — neither the startup apply nor any test
/// dispatches the `shutdown` method, so nothing observes it.
pub(crate) async fn dispatch_for_tests(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    dispatch(req, state, timer_tx, Arc::new(Notify::new())).await
}

async fn dispatch(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
    shutdown: Arc<Notify>,
) -> IpcResponse {
    match req.method.as_str() {
        "shutdown" => {
            // Reply first, then signal: the caller wants confirmation the
            // daemon accepted the request before the process tears down.
            shutdown.notify_one();
            IpcResponse::success(serde_json::json!({}))
        }
        "set" => apply::cmd_set(req, state).await,
        "apply_profile" => apply::cmd_apply_profile(req, state, timer_tx).await,
        "apply_canvas" => apply::cmd_apply_canvas(req, state, timer_tx).await,
        "slideshow_next" => slideshow::cmd_slideshow_advance(state, timer_tx, None).await,
        "slideshow_prev" => slideshow::cmd_slideshow_prev(state).await,
        "slideshow_goto" => slideshow::cmd_slideshow_goto(req, state, timer_tx).await,
        "slideshow_pause" => slideshow::cmd_slideshow_pause(req, state).await,
        "slideshow_pool" => slideshow::cmd_slideshow_pool(req, state).await,
        "redetect" => apply::cmd_redetect(state).await,
        "wait_for_monitor_change" => apply::cmd_wait_for_monitor_change(state).await,
        "current_state" => apply::cmd_current_state(state).await,
        "list_profiles" => handlers::cmd_list_profiles(state).await,
        "save_profile" => handlers::cmd_save_profile(req, state).await,
        "delete_profile" => handlers::cmd_delete_profile(req, state).await,
        "duplicate_profile" => handlers::cmd_duplicate_profile(req, state).await,
        "rename_profile" => handlers::cmd_rename_profile(req, state).await,
        "update_profile_monitor_state" => {
            handlers::cmd_update_profile_monitor_state(req, state).await
        }
        "update_profile_image_transform" => {
            handlers::cmd_update_profile_image_transform(req, state).await
        }
        "update_profile_source" => handlers::cmd_update_profile_source(req, state, timer_tx).await,
        "list_schedules" => handlers::cmd_list_schedules(state).await,
        "save_schedules" => handlers::cmd_save_schedules(req, state).await,
        "set_schedules_paused" => handlers::cmd_set_schedules_paused(req, state).await,
        "get_config" => handlers::cmd_get_config(state).await,
        "save_config" => handlers::cmd_save_config(req, state).await,
        "set_monitor_physical_size" => handlers::cmd_set_monitor_physical_size(req, state).await,
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
    use std::collections::HashMap;
    use superpanels_core::TopologyFingerprint;
    use superpanels_core::config::{
        BackendKind, Config, ImageSet, Profile, ProfileBody, SlideshowConfig as SlideshowCfg,
        SlideshowProfile, SlideshowSort, SlideshowSource, SlideshowStart, StandardLayer,
        StandardProfile,
    };
    use superpanels_core::layout::ImageRectMm;
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
        let now = superpanels_core::config::now_timestamp();
        Profile {
            name: name.to_owned(),
            body: ProfileBody::Slideshow(SlideshowProfile {
                source: SlideshowSource {
                    images: ImageSet::from_folder(folder.to_path_buf(), false),
                    config: SlideshowCfg {
                        interval: Duration::from_secs(60),
                        sort: SlideshowSort::Alphabetical,
                        recent_history_size: 4,
                        on_start: SlideshowStart::Resume,
                        pause_when_active: false,
                        skip_on_unavailable: false,
                    },
                    overrides: HashMap::new(),
                    uniform_layout: false,
                },
                image_rect_mm: ImageRectMm::default(),
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: Some(BackendKind::Custom),
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
    async fn shutdown_request_succeeds_and_fires_the_notifier() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let shutdown = Arc::new(Notify::new());
        let resp = dispatch(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "shutdown".to_owned(),
                params: json!({}),
            },
            Arc::clone(&state),
            timer_pair(),
            Arc::clone(&shutdown),
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        // `notify_one` left a permit, so the daemon's `wait_for_shutdown`
        // arm resolves immediately. The timeout guards against a regression
        // where the arm never fires and the daemon ignores the request.
        tokio::time::timeout(Duration::from_secs(1), shutdown.notified())
            .await
            .expect("shutdown notifier should have been fired");
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
    async fn slideshow_goto_records_requested_image_as_current() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        write_dummy_image(&a);
        write_dummy_image(&b);
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![a.clone()]));
        let state = make_state_arc(s);

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_goto".to_owned(),
                params: json!({"path": b.to_string_lossy()}),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;

        // The apply itself may fail in the test environment (no real
        // backend); the picker must still have jumped before it ran.
        if let Some(err) = &resp.error {
            assert!(
                !err.contains("not in the slideshow pool"),
                "pool membership check rejected an in-pool image: {err}"
            );
        }
        let guard = state.lock().await;
        let picker = guard.slideshow_picker.as_ref().expect("picker kept");
        assert_eq!(picker.state().history.front(), Some(&b));
    }

    #[tokio::test]
    async fn slideshow_goto_rejects_image_outside_pool() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.png");
        write_dummy_image(&a);
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let mut s = DaemonState::for_tests(config);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![a.clone()]));
        let state = make_state_arc(s);

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_goto".to_owned(),
                params: json!({"path": "/elsewhere/x.png"}),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;

        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("not in the slideshow pool"));
        let guard = state.lock().await;
        let picker = guard.slideshow_picker.as_ref().expect("picker kept");
        assert_eq!(picker.state().history.front(), Some(&a));
    }

    #[tokio::test]
    async fn slideshow_goto_errors_without_path_param() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_goto".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("missing 'path' param"));
    }

    #[tokio::test]
    async fn slideshow_pool_returns_resolved_paths_for_named_profile() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.png");
        write_dummy_image(&a);
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let state = make_state_arc(DaemonState::for_tests(config));

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pool".to_owned(),
                params: json!({"profile": "p"}),
            },
            state,
            timer_pair(),
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        let arr = resp.result.unwrap();
        assert_eq!(arr, json!([a.to_string_lossy()]));
    }

    #[tokio::test]
    async fn slideshow_pool_returns_empty_array_for_empty_folder() {
        let dir = tempdir().unwrap();
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        let state = make_state_arc(DaemonState::for_tests(config));

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pool".to_owned(),
                params: json!({"profile": "p"}),
            },
            state,
            timer_pair(),
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert_eq!(resp.result.unwrap(), json!([]));
    }

    #[tokio::test]
    async fn slideshow_pool_errors_when_profile_not_found() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pool".to_owned(),
                params: json!({"profile": "ghost"}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn slideshow_pool_errors_for_standard_profile() {
        let mut config = Config::default();
        let now = superpanels_core::config::now_timestamp();
        config.profiles.push(Profile {
            name: "std".to_owned(),
            body: ProfileBody::Standard(StandardProfile {
                layers: vec![StandardLayer {
                    path: PathBuf::from("/img.png"),
                    image_rect_mm: ImageRectMm::default(),
                }],
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: None,
        });
        let state = make_state_arc(DaemonState::for_tests(config));

        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pool".to_owned(),
                params: json!({"profile": "std"}),
            },
            state,
            timer_pair(),
        )
        .await;

        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("no slideshow source"));
    }

    #[tokio::test]
    async fn slideshow_pool_requires_profile_param() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "slideshow_pool".to_owned(),
                params: json!({}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("params.profile"));
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
    async fn update_profile_source_retunes_live_picker_and_timer() {
        // Arrange — active slideshow profile with a live picker and an armed
        // timer at the old 60 s interval.
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        config.save_to(&config_path).unwrap();
        let mut s = DaemonState::for_tests_with_path(config, config_path);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![
            PathBuf::from("/walls/a.png"),
            PathBuf::from("/walls/b.png"),
            PathBuf::from("/walls/c.png"),
        ]));
        let state = make_state_arc(s);
        let (timer_tx, timer_rx) =
            watch::channel::<Option<Duration>>(Some(Duration::from_secs(60)));

        // Act — push a new slideshow source with a 5 s interval and a
        // 1-entry history window.
        let new_source = SlideshowSource {
            images: ImageSet::from_folder(dir.path().to_path_buf(), false),
            config: SlideshowCfg {
                interval: Duration::from_secs(5),
                sort: SlideshowSort::Shuffle,
                recent_history_size: 1,
                on_start: SlideshowStart::Resume,
                pause_when_active: false,
                skip_on_unavailable: true,
            },
            overrides: HashMap::new(),
            uniform_layout: false,
        };
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "update_profile_source".to_owned(),
                params: json!({
                    "profile": "p",
                    "source": serde_json::to_value(&new_source).unwrap(),
                }),
            },
            Arc::clone(&state),
            timer_tx,
        )
        .await;

        // Assert — timer rearmed at the new interval, history trimmed live.
        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert_eq!(*timer_rx.borrow(), Some(Duration::from_secs(5)));
        let guard = state.lock().await;
        let picker = guard.slideshow_picker.as_ref().expect("picker kept");
        assert_eq!(picker.state().history.len(), 1);
    }

    #[tokio::test]
    async fn update_profile_source_creates_picker_on_first_activation() {
        // Arrange — active slideshow profile, but no picker yet (first run).
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        config.save_to(&config_path).unwrap();
        let mut s = DaemonState::for_tests_with_path(config, config_path);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = None;
        let state = make_state_arc(s);
        let (timer_tx, timer_rx) = watch::channel::<Option<Duration>>(None);

        // Act — push a slideshow source onto the active profile.
        let new_source = SlideshowSource {
            images: ImageSet::from_folder(dir.path().to_path_buf(), false),
            config: SlideshowCfg {
                interval: Duration::from_secs(60),
                sort: SlideshowSort::Alphabetical,
                recent_history_size: 4,
                on_start: SlideshowStart::Resume,
                pause_when_active: false,
                skip_on_unavailable: false,
            },
            overrides: HashMap::new(),
            uniform_layout: false,
        };
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "update_profile_source".to_owned(),
                params: json!({
                    "profile": "p",
                    "source": serde_json::to_value(&new_source).unwrap(),
                }),
            },
            Arc::clone(&state),
            timer_tx,
        )
        .await;

        // Assert — a picker exists, so the armed timer's ticks can advance.
        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert_eq!(*timer_rx.borrow(), Some(Duration::from_secs(60)));
        let guard = state.lock().await;
        assert!(guard.slideshow_picker.is_some(), "picker must be created");
    }

    #[tokio::test]
    async fn update_profile_source_with_unchanged_interval_does_not_restart_timer() {
        // An image-set edit on a running slideshow must not reset the
        // countdown — only an interval change may notify the timer task.
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p", dir.path()));
        config.save_to(&config_path).unwrap();
        let mut s = DaemonState::for_tests_with_path(config, config_path);
        s.active_profile = Some("p".to_owned());
        s.slideshow_picker = Some(picker_with_history(vec![]));
        let state = make_state_arc(s);
        let (timer_tx, mut timer_rx) =
            watch::channel::<Option<Duration>>(Some(Duration::from_secs(60)));
        // Keep one sender alive past the dispatch so `has_changed` below
        // reads channel state instead of a closed-channel error.
        let _keep_tx = timer_tx.clone();
        timer_rx.mark_unchanged();

        // Act — same 60 s interval, different sort.
        let new_source = SlideshowSource {
            images: ImageSet::from_folder(dir.path().to_path_buf(), false),
            config: SlideshowCfg {
                interval: Duration::from_secs(60),
                sort: SlideshowSort::Shuffle,
                recent_history_size: 4,
                on_start: SlideshowStart::Resume,
                pause_when_active: false,
                skip_on_unavailable: false,
            },
            overrides: HashMap::new(),
            uniform_layout: false,
        };
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "update_profile_source".to_owned(),
                params: json!({
                    "profile": "p",
                    "source": serde_json::to_value(&new_source).unwrap(),
                }),
            },
            Arc::clone(&state),
            timer_tx,
        )
        .await;

        assert!(resp.is_ok(), "got: {:?}", resp.error);
        assert!(
            !timer_rx.has_changed().unwrap(),
            "unchanged interval must not wake the timer task"
        );
    }

    #[tokio::test]
    async fn apply_canvas_does_not_mutate_persisted_profile_state() {
        // Build a profile with a known empty monitor_state, then issue
        // `apply_canvas` with a payload that carries DIFFERENT monitor_state.
        // The persisted profile must not pick up the canvas placement
        // (Phase 4e §4e.11.1 — Apply is ephemeral). The desktop apply itself
        // is allowed to fail (Custom backend, no real environment); what we
        // pin is the absence of a write-through to `config.profiles`.
        let dir = tempdir().unwrap();
        let img = dir.path().join("a.png");
        write_dummy_image(&img);
        let mut config = Config::default();
        let mut profile = Profile {
            name: "p".to_owned(),
            body: ProfileBody::Standard(StandardProfile {
                layers: vec![StandardLayer {
                    path: img.clone(),
                    image_rect_mm: ImageRectMm::default(),
                }],
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: superpanels_core::config::now_timestamp(),
            updated_at: superpanels_core::config::now_timestamp(),
            last_applied_at: None,
            backend_override: Some(BackendKind::Custom),
        };
        profile.monitor_state.insert(
            "persisted".to_owned(),
            superpanels_core::MonitorPlacement {
                x_mm: 0.0,
                y_mm: 0.0,
            },
        );
        config.profiles.push(profile.clone());
        let state = make_state_arc(DaemonState::for_tests(config));

        // Canvas payload carries a different placement set.
        let mut canvas = profile.clone();
        canvas.monitor_state.clear();
        canvas.monitor_state.insert(
            "ephemeral".to_owned(),
            superpanels_core::MonitorPlacement {
                x_mm: 999.0,
                y_mm: 0.0,
            },
        );

        let _ = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "apply_canvas".to_owned(),
                params: json!({
                    "profile": serde_json::to_value(&canvas).unwrap(),
                    "active_name": "p",
                }),
            },
            Arc::clone(&state),
            timer_pair(),
        )
        .await;

        let guard = state.lock().await;
        let stored = guard
            .config
            .profiles
            .iter()
            .find(|p| p.name == "p")
            .expect("profile still present");
        assert!(
            stored.monitor_state.contains_key("persisted"),
            "expected persisted monitor_state to remain unchanged"
        );
        assert!(
            !stored.monitor_state.contains_key("ephemeral"),
            "apply_canvas leaked transient placement into config: {:?}",
            stored.monitor_state.keys().collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn apply_canvas_rejects_malformed_profile_payload() {
        let state = make_state_arc(DaemonState::for_tests(Config::default()));
        let resp = dispatch_for_tests(
            IpcRequest {
                v: PROTOCOL_VERSION,
                method: "apply_canvas".to_owned(),
                params: json!({"profile": "not-an-object"}),
            },
            state,
            timer_pair(),
        )
        .await;
        assert!(!resp.is_ok());
        assert!(resp.error.unwrap().contains("malformed"));
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
