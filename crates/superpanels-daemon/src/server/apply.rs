//! Apply / set / redetect / current-state IPC handlers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::backends::AppliedReport;
use superpanels_core::config::{ProfileBody, SpanSource};
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::schedule::MonitorPlacement;
use tokio::sync::{Mutex, watch};
use tracing::info;

use crate::apply::{
    run_composite_apply, run_immediate_span_apply, run_per_monitor_apply, run_span_apply,
};
use crate::pool::resolve_pool;
use crate::state::DaemonState;

/// Run a blocking apply closure on the pool, then (when `active_name` is set)
/// rotate the active profile and restart the timer. Maps join / apply errors to
/// an `IpcResponse` failure so the non-span branches stay a single early return.
async fn apply_and_finish<F>(
    state: &Arc<Mutex<DaemonState>>,
    timer_tx: &watch::Sender<Option<Duration>>,
    active_name: Option<&str>,
    f: F,
) -> IpcResponse
where
    F: FnOnce() -> anyhow::Result<AppliedReport> + Send + 'static,
{
    let report = match tokio::task::spawn_blocking(f).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return IpcResponse::failure(format!("{e:#}")),
        Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
    };
    if let Some(name) = active_name {
        update_active_profile(state, name, report.backend).await;
        restart_timer(state, timer_tx).await;
    }
    IpcResponse::success(applied_json(&report))
}

use super::helpers::{applied_json, init_picker_if_needed, restart_timer, update_active_profile};

pub(super) async fn cmd_set(req: IpcRequest, state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let image_path = match req.params.get("image").and_then(|v| v.as_str()) {
        Some(s) => PathBuf::from(s),
        None => return IpcResponse::failure("params.image (string) required"),
    };

    let (monitors, backend_kind, custom_cmd) = {
        let guard = state.lock().await;
        (
            guard.monitors.clone(),
            guard.config.backend.prefer,
            guard.config.backend.custom_command.clone(),
        )
    };

    let placements: HashMap<String, MonitorPlacement> = HashMap::new();

    let report = tokio::task::spawn_blocking(move || {
        run_immediate_span_apply(
            &image_path,
            &monitors,
            &placements,
            None,
            backend_kind,
            &custom_cmd,
        )
    })
    .await;
    match report {
        Ok(Ok(r)) => IpcResponse::success(applied_json(&r)),
        Ok(Err(e)) => IpcResponse::failure(format!("{e:#}")),
        Err(e) => IpcResponse::failure(format!("task panic: {e}")),
    }
}

pub(super) async fn cmd_apply_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    let name = match req.params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_owned(),
        None => return IpcResponse::failure("params.name (string) required"),
    };

    let (profile, monitors, backend_kind, custom_cmd) = {
        let guard = state.lock().await;
        let Some(profile) = guard
            .config
            .profiles
            .iter()
            .find(|p| p.name == name)
            .cloned()
        else {
            return IpcResponse::failure(format!("profile '{name}' not found"));
        };
        let monitors = guard.monitors.clone();
        // Skip validity gate during the transient empty-monitors state (e.g.
        // detection hasn't run yet, or test fixtures); layout will surface a
        // clearer error if it actually matters.
        if !monitors.is_empty() {
            let validity = superpanels_core::ProfileValidity::evaluate(&profile, &monitors);
            if let superpanels_core::ProfileValidity::Disabled { reasons } = validity {
                return IpcResponse::failure(format!(
                    "profile '{name}' is disabled: {}",
                    serde_json::to_string(&reasons).unwrap_or_default()
                ));
            }
        }
        let backend_kind = profile
            .backend_override
            .unwrap_or(guard.config.backend.prefer);
        let custom_cmd = guard.config.backend.custom_command.clone();
        (profile, monitors, backend_kind, custom_cmd)
    };

    if let ProfileBody::PerMonitor(pm) = &profile.body {
        let assignments = pm.assignments.clone();
        let fit = pm.fit;
        return apply_and_finish(&state, &timer_tx, Some(&name), move || {
            run_per_monitor_apply(&assignments, &monitors, fit, backend_kind, &custom_cmd)
        })
        .await;
    }

    if let ProfileBody::Composite(composite) = &profile.body {
        let layers = composite.layers.clone();
        let placements = profile.monitor_state.clone();
        return apply_and_finish(&state, &timer_tx, Some(&name), move || {
            run_composite_apply(&layers, &monitors, &placements, backend_kind, &custom_cmd)
        })
        .await;
    }

    let image_path = match &profile.body {
        ProfileBody::Span(span) => match &span.source {
            SpanSource::Single { path } => path.clone(),
            SpanSource::Slideshow { images, .. } => {
                let Some(pool) = resolve_pool(&state, images).await else {
                    return IpcResponse::failure("slideshow pool is empty");
                };
                let mut guard = state.lock().await;
                // A picker left over from a different profile carries that
                // profile's config and history — start fresh instead.
                if guard.active_profile.as_deref() != Some(name.as_str()) {
                    guard.slideshow_picker = None;
                }
                init_picker_if_needed(&mut guard, &name);
                match guard
                    .slideshow_picker
                    .as_mut()
                    .and_then(|p| p.next(&pool).ok())
                {
                    Some(p) => p,
                    None => return IpcResponse::failure("slideshow pool is empty"),
                }
            }
        },
        ProfileBody::PerMonitor(_) | ProfileBody::Composite(_) => unreachable!("handled above"),
    };

    let profile_clone = profile.clone();
    let monitors_clone = monitors.clone();
    let report = tokio::task::spawn_blocking(move || {
        run_span_apply(
            &image_path,
            &monitors_clone,
            &profile_clone,
            backend_kind,
            &custom_cmd,
        )
    })
    .await;
    let report = match report {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return IpcResponse::failure(e.to_string()),
        Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
    };

    update_active_profile(&state, &name, report.backend).await;
    restart_timer(&state, &timer_tx).await;
    IpcResponse::success(applied_json(&report))
}

/// Push a transient canvas payload to the desktop without persisting it.
/// Mirrors the apply pipeline
/// of `cmd_apply_profile` but takes the profile as an in-memory payload so
/// the live config is left untouched. If `params.active_name` matches a
/// stored profile, that profile becomes the active one and its
/// `last_applied_at` is bumped — but `monitor_state`, image transform, and
/// source on disk are preserved.
pub(super) async fn cmd_apply_canvas(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    let Some(profile_val) = req.params.get("profile") else {
        return IpcResponse::failure("params.profile required");
    };
    let profile: superpanels_core::Profile = match serde_json::from_value(profile_val.clone()) {
        Ok(p) => p,
        Err(e) => return IpcResponse::failure(format!("profile is malformed: {e}")),
    };
    let active_name = req
        .params
        .get("active_name")
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    let (monitors, backend_kind, custom_cmd) = {
        let guard = state.lock().await;
        let backend_kind = profile
            .backend_override
            .unwrap_or(guard.config.backend.prefer);
        (
            guard.monitors.clone(),
            backend_kind,
            guard.config.backend.custom_command.clone(),
        )
    };

    if let ProfileBody::PerMonitor(pm) = &profile.body {
        let assignments = pm.assignments.clone();
        let fit = pm.fit;
        return apply_and_finish(&state, &timer_tx, active_name.as_deref(), move || {
            run_per_monitor_apply(&assignments, &monitors, fit, backend_kind, &custom_cmd)
        })
        .await;
    }

    if let ProfileBody::Composite(composite) = &profile.body {
        // The canvas is the source of truth: composite straight from the
        // payload's layers + placements, like the span branch below.
        let layers = composite.layers.clone();
        let placements = profile.monitor_state.clone();
        return apply_and_finish(&state, &timer_tx, active_name.as_deref(), move || {
            run_composite_apply(&layers, &monitors, &placements, backend_kind, &custom_cmd)
        })
        .await;
    }

    let (image_path, image_rect_mm) = match &profile.body {
        ProfileBody::Span(span) => {
            let path = match &span.source {
                SpanSource::Single { path } => path.clone(),
                SpanSource::Slideshow { images, .. } => {
                    let Some(pool) = resolve_pool(&state, images).await else {
                        return IpcResponse::failure("slideshow pool is empty");
                    };
                    let resolved = {
                        let guard = state.lock().await;
                        canvas_slideshow_image(&guard, active_name.as_deref(), &pool)
                    };
                    let Some(path) = resolved else {
                        return IpcResponse::failure("slideshow pool is empty");
                    };
                    path
                }
            };
            (path, span.image_rect_mm)
        }
        ProfileBody::PerMonitor(_) | ProfileBody::Composite(_) => unreachable!("handled above"),
    };

    // The canvas is the source of truth for this apply: use the payload's
    // top-level layout directly. Funnelling through `run_span_apply` would
    // let a stored per-image override shadow the user's live canvas edits.
    let placements = profile.monitor_state.clone();
    let monitors_clone = monitors.clone();
    let report = tokio::task::spawn_blocking(move || {
        run_immediate_span_apply(
            &image_path,
            &monitors_clone,
            &placements,
            Some(image_rect_mm),
            backend_kind,
            &custom_cmd,
        )
    })
    .await;
    let report = match report {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return IpcResponse::failure(e.to_string()),
        Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
    };

    if let Some(name) = active_name.as_deref() {
        update_active_profile(&state, name, report.backend).await;
        restart_timer(&state, &timer_tx).await;
    }
    IpcResponse::success(applied_json(&report))
}

/// The image a canvas apply targets for a slideshow source: the image on
/// screen (the active picker's newest history entry) when the canvas belongs
/// to the active profile and that image is still in the pool, else the pool's
/// first image. Applying the canvas must not re-show a different image than
/// the one the user is looking at.
fn canvas_slideshow_image(
    state: &DaemonState,
    active_name: Option<&str>,
    pool: &[PathBuf],
) -> Option<PathBuf> {
    let on_screen = active_name
        .filter(|n| state.active_profile.as_deref() == Some(n))
        .and(state.slideshow_picker.as_ref())
        .and_then(|p| p.state().history.front().cloned())
        .filter(|p| pool.contains(p));
    on_screen.or_else(|| pool.first().cloned())
}

/// 55 s safety timeout, well under the IPC client's 120 s read timeout.
const WAIT_FOR_MONITOR_CHANGE_TIMEOUT: Duration = Duration::from_secs(55);

/// Long-poll: block until `display_watch` broadcasts a monitor-config change
/// (or the safety timeout fires).
/// Returns `{ "changed": bool }`; clients re-issue immediately to resume the
/// subscription. The channel carries no payload — the GUI calls
/// `detect_monitors` on a `true` response to pull the fresh snapshot.
pub(super) async fn cmd_wait_for_monitor_change(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    wait_for_monitor_change_inner(&state, WAIT_FOR_MONITOR_CHANGE_TIMEOUT).await
}

async fn wait_for_monitor_change_inner(
    state: &Arc<Mutex<DaemonState>>,
    timeout: Duration,
) -> IpcResponse {
    let tx_opt = {
        let guard = state.lock().await;
        guard.monitors_tx.clone()
    };
    let Some(tx) = tx_opt else {
        return IpcResponse::failure("OS-rotation push channel not initialised");
    };
    let mut rx = tx.subscribe();
    match tokio::time::timeout(timeout, rx.recv()).await {
        // Real tick OR a `Lagged`/`Closed` recv error — both mean we should
        // re-check, so report `changed: true` and let the client refresh.
        Ok(_) => IpcResponse::success(json!({ "changed": true })),
        Err(_elapsed) => IpcResponse::success(json!({ "changed": false })),
    }
}

// Deliberately does NOT publish on `monitors_tx`: this is a GUI-initiated
// refresh, the caller already updates its view from the response, and the
// push relay's `monitors://changed` listener would loop straight back into
// `detect_monitors` (which calls `redetect` first) → publish → event → …
pub(super) async fn cmd_redetect(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let mut guard = state.lock().await;
    match superpanels_core::detect(None) {
        Ok(mut monitors) => {
            guard.config.merge_into_monitors(&mut monitors);
            let count = monitors.len();
            guard.monitors = monitors;
            info!(monitors = count, "monitors re-detected");
            IpcResponse::success(json!({"monitors": count}))
        }
        Err(e) => IpcResponse::failure(e.to_string()),
    }
}

pub(super) async fn cmd_current_state(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    IpcResponse::success(guard.to_runtime_state())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected errors
mod tests {
    use super::*;
    use superpanels_core::config::Config;
    use tokio::sync::broadcast;

    fn state_with_tx(tx: broadcast::Sender<()>) -> Arc<Mutex<DaemonState>> {
        let mut ds = DaemonState::for_tests(Config::default());
        ds.monitors_tx = Some(tx);
        Arc::new(Mutex::new(ds))
    }

    fn state_showing(active: Option<&str>, on_screen: Option<&str>) -> DaemonState {
        use superpanels_core::slideshow::{
            SlideshowConfig, SlideshowPicker, SlideshowSort, SlideshowStart,
        };
        let mut ds = DaemonState::for_tests(Config::default());
        ds.active_profile = active.map(str::to_owned);
        if let Some(path) = on_screen {
            let mut picker = SlideshowPicker::new(SlideshowConfig {
                interval: Duration::from_secs(60),
                sort: SlideshowSort::Alphabetical,
                recent_history_size: 4,
                on_start: SlideshowStart::Resume,
                pause_when_active: false,
                skip_on_unavailable: false,
            });
            picker.state_mut().history.push_front(PathBuf::from(path));
            ds.slideshow_picker = Some(picker);
        }
        ds
    }

    fn pool() -> Vec<PathBuf> {
        vec![PathBuf::from("/walls/a.png"), PathBuf::from("/walls/b.png")]
    }

    #[test]
    fn canvas_slideshow_image_prefers_on_screen_image() {
        let ds = state_showing(Some("p"), Some("/walls/b.png"));
        let resolved = canvas_slideshow_image(&ds, Some("p"), &pool());
        assert_eq!(resolved, Some(PathBuf::from("/walls/b.png")));
    }

    #[test]
    fn canvas_slideshow_image_falls_back_when_profile_not_active() {
        let ds = state_showing(Some("other"), Some("/walls/b.png"));
        let resolved = canvas_slideshow_image(&ds, Some("p"), &pool());
        assert_eq!(resolved, Some(PathBuf::from("/walls/a.png")));
    }

    #[test]
    fn canvas_slideshow_image_falls_back_when_on_screen_left_the_pool() {
        let ds = state_showing(Some("p"), Some("/walls/removed.png"));
        let resolved = canvas_slideshow_image(&ds, Some("p"), &pool());
        assert_eq!(resolved, Some(PathBuf::from("/walls/a.png")));
    }

    #[test]
    fn canvas_slideshow_image_falls_back_without_active_name() {
        let ds = state_showing(Some("p"), Some("/walls/b.png"));
        let resolved = canvas_slideshow_image(&ds, None, &pool());
        assert_eq!(resolved, Some(PathBuf::from("/walls/a.png")));
    }

    #[tokio::test]
    async fn wait_for_monitor_change_fails_when_channel_uninitialised() {
        let state = Arc::new(Mutex::new(DaemonState::for_tests(Config::default())));
        let resp = wait_for_monitor_change_inner(&state, Duration::from_millis(50)).await;
        assert!(!resp.is_ok());
        let err = resp.error.unwrap_or_default();
        assert!(err.contains("not initialised"), "got: {err}");
    }

    #[tokio::test]
    async fn redetect_does_not_publish_on_monitors_tx() {
        // Regression: a publish here forms a feedback loop with the GUI push
        // relay, because the GUI's `detect_monitors` command (which fires on
        // `monitors://changed`) calls daemon `redetect` first.
        let (tx, mut rx) = broadcast::channel::<()>(4);
        // Keep one sender alive outside `state` so dropping `state` doesn't
        // turn an empty channel into a `Closed` error.
        let _keep_tx = tx.clone();
        let state = state_with_tx(tx);
        let _resp = cmd_redetect(state).await;
        assert!(
            matches!(rx.try_recv(), Err(broadcast::error::TryRecvError::Empty)),
            "cmd_redetect must not publish on monitors_tx"
        );
    }

    #[tokio::test]
    async fn wait_for_monitor_change_returns_changed_true_on_tick() {
        let (tx, _keep) = broadcast::channel::<()>(4);
        let state = state_with_tx(tx.clone());

        let tx_publish = tx.clone();
        tokio::spawn(async move {
            // Brief sleep so the handler subscribes before we send. A
            // pre-subscribe send would be missed by broadcast's tail-only
            // semantics for new receivers.
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx_publish.send(());
        });

        let resp = wait_for_monitor_change_inner(&state, Duration::from_secs(2)).await;
        assert!(resp.is_ok(), "got error: {:?}", resp.error);
        let result = resp.result.unwrap();
        assert_eq!(result["changed"], json!(true));
    }

    #[tokio::test]
    async fn wait_for_monitor_change_returns_changed_false_on_timeout() {
        let (tx, _keep) = broadcast::channel::<()>(4);
        let state = state_with_tx(tx);
        let resp = wait_for_monitor_change_inner(&state, Duration::from_millis(50)).await;
        assert!(resp.is_ok(), "got error: {:?}", resp.error);
        let result = resp.result.unwrap();
        assert_eq!(result["changed"], json!(false));
    }
}
