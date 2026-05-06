//! Apply / set / redetect / current-state IPC handlers (`SPEC §12.4`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::config::{ProfileBody, SpanSource};
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::layout::{BezelConfig, FitMode};
use tokio::sync::{Mutex, watch};
use tracing::info;

use crate::apply::{run_immediate_set_with_offset, run_per_monitor_apply, run_span_apply};
use crate::state::DaemonState;

use super::helpers::{
    applied_json, init_picker_if_needed, resolve_pool, update_active_profile, update_timer,
};

pub(super) async fn cmd_set(req: IpcRequest, state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let image_path = match req.params.get("image").and_then(|v| v.as_str()) {
        Some(s) => PathBuf::from(s),
        None => return IpcResponse::failure("params.image (string) required"),
    };
    let bezel_h: f32 = req
        .params
        .get("bezel_h")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(0.0_f32);
    let bezel_v: f32 = req
        .params
        .get("bezel_v")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(0.0_f32);
    let fit: FitMode = req
        .params
        .get("fit")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let offset_px: [i32; 2] = req
        .params
        .get("offset_px")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or([0, 0]);

    let (monitors, backend_kind, custom_cmd) = {
        let guard = state.lock().await;
        (
            guard.monitors.clone(),
            guard.config.backend.prefer,
            guard.config.backend.custom_command.clone(),
        )
    };
    let bezels = BezelConfig {
        horizontal_mm: bezel_h,
        vertical_mm: bezel_v,
    };

    let report = tokio::task::spawn_blocking(move || {
        // CLI-equivalent `set` doesn't carry image_size_px (`docs/plan/phase-4c-free-positioning.md`
        // §4c.9); GUI free-transform always goes through `apply_profile`.
        run_immediate_set_with_offset(
            &image_path,
            &monitors,
            bezels,
            fit,
            offset_px,
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
