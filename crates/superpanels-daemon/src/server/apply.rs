//! Apply / set / redetect / current-state IPC handlers (`SPEC §12.4`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::config::{ProfileBody, SpanSource};
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::schedule::MonitorPlacement;
use tokio::sync::{Mutex, watch};
use tracing::info;

use crate::apply::{run_immediate_span_apply, run_per_monitor_apply, run_span_apply};
use crate::state::DaemonState;

use super::helpers::{
    applied_json, init_picker_if_needed, resolve_pool, update_active_profile, update_timer,
};

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
        let report = tokio::task::spawn_blocking(move || {
            run_per_monitor_apply(&assignments, &monitors, fit, backend_kind, &custom_cmd)
        })
        .await;
        let report = match report {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return IpcResponse::failure(format!("{e:#}")),
            Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
        };
        update_active_profile(&state, &name).await;
        update_timer(&state, &timer_tx).await;
        return IpcResponse::success(applied_json(&report));
    }

    let image_path = match &profile.body {
        ProfileBody::Span(span) => match &span.source {
            SpanSource::Single { path } => path.clone(),
            SpanSource::Slideshow { images, .. } => {
                let Some(pool) = resolve_pool(&state, images).await else {
                    return IpcResponse::failure("slideshow pool is empty");
                };
                let mut guard = state.lock().await;
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
        ProfileBody::PerMonitor(_) => unreachable!("handled above"),
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

    update_active_profile(&state, &name).await;
    update_timer(&state, &timer_tx).await;
    IpcResponse::success(applied_json(&report))
}

/// Push a transient canvas payload to the desktop without persisting it
/// (`docs/spec/09-profiles-schedules.md` §9.1.2). Mirrors the apply pipeline
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
        let report = tokio::task::spawn_blocking(move || {
            run_per_monitor_apply(&assignments, &monitors, fit, backend_kind, &custom_cmd)
        })
        .await;
        let report = match report {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return IpcResponse::failure(format!("{e:#}")),
            Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
        };
        if let Some(name) = active_name.as_deref() {
            update_active_profile(&state, name).await;
            update_timer(&state, &timer_tx).await;
        }
        return IpcResponse::success(applied_json(&report));
    }

    let image_path = match &profile.body {
        ProfileBody::Span(span) => match &span.source {
            SpanSource::Single { path } => path.clone(),
            SpanSource::Slideshow { images, .. } => {
                let Some(pool) = resolve_pool(&state, images).await else {
                    return IpcResponse::failure("slideshow pool is empty");
                };
                let Some(first) = pool.into_iter().next() else {
                    return IpcResponse::failure("slideshow pool is empty");
                };
                first
            }
        },
        ProfileBody::PerMonitor(_) => unreachable!("handled above"),
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

    if let Some(name) = active_name.as_deref() {
        update_active_profile(&state, name).await;
        update_timer(&state, &timer_tx).await;
    }
    IpcResponse::success(applied_json(&report))
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
