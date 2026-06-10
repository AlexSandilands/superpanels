//! Slideshow next/prev/goto/pause IPC handlers.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::config::{ProfileBody, SpanSource};
use superpanels_core::ipc::{IpcRequest, IpcResponse};
use superpanels_core::slideshow::SlideshowPicker;
use tokio::sync::{Mutex, watch};
use tracing::info;

use crate::apply::run_span_apply;
use crate::pool::resolve_pool;
use crate::state::DaemonState;

use super::helpers::{applied_json, init_picker_if_needed, restart_timer, update_active_profile};

/// Advance to the picker's next image, or — when `goto` is set — jump
/// straight to that image (it must be in the resolved pool).
pub(super) async fn cmd_slideshow_advance(
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
    goto: Option<PathBuf>,
) -> IpcResponse {
    let snapshot = {
        let guard = state.lock().await;
        let Some(name) = guard.active_profile.clone() else {
            return IpcResponse::failure("no active profile");
        };
        let Some(profile) = guard
            .config
            .profiles
            .iter()
            .find(|p| p.name == name)
            .cloned()
        else {
            return IpcResponse::failure("active profile no longer in config");
        };
        let images = match &profile.body {
            ProfileBody::Span(span) => match &span.source {
                SpanSource::Slideshow { images, .. } => images.clone(),
                SpanSource::Single { .. } => {
                    return IpcResponse::failure("active profile has no slideshow");
                }
            },
            ProfileBody::PerMonitor(_) => {
                return IpcResponse::failure("active profile has no slideshow");
            }
        };
        let backend_kind = profile
            .backend_override
            .unwrap_or(guard.config.backend.prefer);
        let custom_cmd = guard.config.backend.custom_command.clone();
        let monitors = guard.monitors.clone();
        (profile, monitors, backend_kind, custom_cmd, name, images)
    };
    let (profile, monitors, backend_kind, custom_cmd, name, images) = snapshot;

    let Some(pool) = resolve_pool(&state, &images).await else {
        return IpcResponse::failure("slideshow pool is empty");
    };

    let picked = {
        let mut guard = state.lock().await;
        init_picker_if_needed(&mut guard, &name);
        let Some(picker) = guard.slideshow_picker.as_mut() else {
            return IpcResponse::failure("active profile has no slideshow");
        };
        match goto {
            Some(path) => picker.jump_to(&pool, &path).map(|()| path),
            None => picker.next(&pool),
        }
    };

    let image_path = match picked {
        Ok(path) => path,
        Err(e) => return IpcResponse::failure(e.to_string()),
    };

    let report = tokio::task::spawn_blocking(move || {
        run_span_apply(&image_path, &monitors, &profile, backend_kind, &custom_cmd)
    })
    .await;
    let report = match report {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return IpcResponse::failure(e.to_string()),
        Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
    };

    update_active_profile(&state, &name).await;
    restart_timer(&state, &timer_tx).await;
    IpcResponse::success(applied_json(&report))
}

pub(super) async fn cmd_slideshow_goto(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    let Some(path) = req.params.get("path").and_then(serde_json::Value::as_str) else {
        return IpcResponse::failure("missing 'path' param");
    };
    cmd_slideshow_advance(state, timer_tx, Some(PathBuf::from(path))).await
}

pub(super) async fn cmd_slideshow_prev(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let (profile, monitors, backend_kind, custom_cmd, image_path) = {
        let mut guard = state.lock().await;
        let Some(name) = guard.active_profile.clone() else {
            return IpcResponse::failure("no active profile");
        };
        let Some(profile) = guard
            .config
            .profiles
            .iter()
            .find(|p| p.name == name)
            .cloned()
        else {
            return IpcResponse::failure("active profile no longer in config");
        };
        match &profile.body {
            ProfileBody::Span(span) => match &span.source {
                SpanSource::Slideshow { .. } => {}
                SpanSource::Single { .. } => {
                    return IpcResponse::failure("active profile has no slideshow");
                }
            },
            ProfileBody::PerMonitor(_) => {
                return IpcResponse::failure("active profile has no slideshow");
            }
        }
        let backend_kind = profile
            .backend_override
            .unwrap_or(guard.config.backend.prefer);
        let custom_cmd = guard.config.backend.custom_command.clone();
        let monitors = guard.monitors.clone();
        // Mutates history so `current_path` tracks what's on screen.
        let path = guard
            .slideshow_picker
            .as_mut()
            .and_then(SlideshowPicker::step_back);
        (profile, monitors, backend_kind, custom_cmd, path)
    };

    let Some(image_path) = image_path else {
        return IpcResponse::failure("no previous image in history");
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
    match report {
        Ok(Ok(r)) => IpcResponse::success(applied_json(&r)),
        Ok(Err(e)) => IpcResponse::failure(e.to_string()),
        Err(e) => IpcResponse::failure(format!("task panic: {e}")),
    }
}

pub(super) async fn cmd_slideshow_pause(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
) -> IpcResponse {
    let explicit = req
        .params
        .get("paused")
        .and_then(serde_json::Value::as_bool);
    let mut guard = state.lock().await;
    if let Some(picker) = guard.slideshow_picker.as_mut() {
        let new_paused = explicit.unwrap_or(!picker.state().paused);
        picker.state_mut().paused = new_paused;
        info!(paused = new_paused, "slideshow pause state changed");
        IpcResponse::success(json!({"paused": new_paused}))
    } else {
        IpcResponse::failure("no active slideshow")
    }
}
