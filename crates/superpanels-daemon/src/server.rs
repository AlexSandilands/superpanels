//! IPC server: accepts Unix socket connections, dispatches JSON requests.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde_json::json;
use superpanels_core::backends::AppliedReport;
use superpanels_core::config::{ImageSet, ProfileBody, SpanSource};
use superpanels_core::ipc::{IpcRequest, IpcResponse, PROTOCOL_VERSION};
use superpanels_core::layout::{BezelConfig, FitMode};
use superpanels_core::slideshow::SlideshowPicker;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, watch};
use tracing::{debug, error, info};

use crate::apply::{
    profile_to_picker_config, run_immediate_set, run_per_monitor_apply, run_span_apply,
};
use crate::pool::{pool_from_cache, scan_blocking};
use crate::state::DaemonState;

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
        let frame = match read_frame(&mut stream).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let req: IpcRequest = match serde_json::from_slice(&frame) {
            Ok(r) => r,
            Err(e) => {
                let resp = IpcResponse::failure(format!("malformed request: {e}"));
                write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
                continue;
            }
        };
        if req.v != PROTOCOL_VERSION {
            let resp = IpcResponse::failure(format!(
                "unsupported protocol version {}; expected {}",
                req.v, PROTOCOL_VERSION
            ));
            write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
            continue;
        }
        debug!(method = %req.method, "IPC request");
        let resp = dispatch(req, Arc::clone(&state), timer_tx.clone()).await;
        write_frame(&mut stream, &serde_json::to_vec(&resp)?).await?;
    }
}

/// Exposed for the startup default-profile apply in `main.rs`.
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
        "set" => cmd_set(req, state).await,
        "apply_profile" => cmd_apply_profile(req, state, timer_tx).await,
        "slideshow_next" => cmd_slideshow_advance(state, timer_tx, false).await,
        "slideshow_prev" => cmd_slideshow_prev(state).await,
        "slideshow_pause" => cmd_slideshow_pause(req, state).await,
        "redetect" => cmd_redetect(state).await,
        "current_state" => cmd_current_state(state).await,
        other => IpcResponse::failure(format!("unknown method: {other}")),
    }
}

async fn cmd_set(req: IpcRequest, state: Arc<Mutex<DaemonState>>) -> IpcResponse {
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
        run_immediate_set(
            &image_path,
            &monitors,
            bezels,
            fit,
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

async fn cmd_apply_profile(
    req: IpcRequest,
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) -> IpcResponse {
    let name = match req.params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_owned(),
        None => return IpcResponse::failure("params.name (string) required"),
    };

    // Extract everything needed for the apply from state (under lock).
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

    // For PerMonitor body, handle separately and return early.
    if let ProfileBody::PerMonitor(pm) = &profile.body {
        let assignments = pm.assignments.clone();
        let fit = pm.fit;
        let report = tokio::task::spawn_blocking(move || {
            run_per_monitor_apply(&assignments, &monitors, fit, backend_kind, &custom_cmd)
        })
        .await;
        let report = match report {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return IpcResponse::failure(e.to_string()),
            Err(e) => return IpcResponse::failure(format!("task panic: {e}")),
        };
        update_active_profile(&state, &name).await;
        update_timer(&state, &timer_tx).await;
        return IpcResponse::success(applied_json(&report));
    }

    // Determine the image to apply for Span profiles.
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

    // Run the span apply pipeline in a blocking thread.
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

async fn cmd_slideshow_advance(
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
    is_prev: bool,
) -> IpcResponse {
    use superpanels_core::config::SpanSource;

    // Snapshot the inputs we need without holding the lock across any blocking work.
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

    // Resolve the slideshow pool with the lock dropped.
    let Some(pool) = resolve_pool(&state, &images).await else {
        return IpcResponse::failure("slideshow pool is empty");
    };

    // Briefly re-acquire the lock to advance the picker.
    let image_path = {
        let mut guard = state.lock().await;
        init_picker_if_needed(&mut guard, &name);
        if is_prev {
            guard
                .slideshow_picker
                .as_ref()
                .and_then(|p| p.state().history.get(1).cloned())
        } else {
            guard
                .slideshow_picker
                .as_mut()
                .and_then(|p| p.next(&pool).ok())
        }
    };

    let Some(image_path) = image_path else {
        return IpcResponse::failure("slideshow pool is empty or no previous image");
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
    // Reset timer so the interval counts from this manual advance.
    update_timer(&state, &timer_tx).await;
    IpcResponse::success(applied_json(&report))
}

async fn cmd_slideshow_prev(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    // A simple implementation: apply the most recently shown image (history[1]).
    // We don't reset the timer here — let the existing interval continue.

    let (profile, monitors, backend_kind, custom_cmd, image_path) = {
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
        let path = guard
            .slideshow_picker
            .as_ref()
            .and_then(|p| p.state().history.get(1).cloned());
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

async fn cmd_slideshow_pause(req: IpcRequest, state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    // Accepts an optional `{"paused": bool}` param. Without it, toggles.
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

async fn cmd_redetect(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
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

async fn cmd_current_state(state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    let guard = state.lock().await;
    IpcResponse::success(guard.to_runtime_state())
}

// --- helpers ---

/// Resolve `images` to a list of paths without holding `state` locked across
/// any disk walk. Tries the cached library first; if that misses, runs
/// `scan_folder` in `spawn_blocking` and updates the cache on completion.
/// Returns `None` only when the resulting pool is empty.
async fn resolve_pool(state: &Arc<Mutex<DaemonState>>, images: &ImageSet) -> Option<Vec<PathBuf>> {
    // 1. Read cache under the lock and drop the guard.
    let cached = {
        let guard = state.lock().await;
        pool_from_cache(images, &guard.library)
    };
    if let Some(pool) = cached {
        if !pool.is_empty() {
            return Some(pool);
        }
    }

    // 2. Cache miss / empty — scan the disk with the lock dropped.
    let images_clone = images.clone();
    let scanned = match tokio::task::spawn_blocking(move || scan_blocking(&images_clone)).await {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "pool resolver task panicked");
            return None;
        }
    };
    if scanned.is_empty() {
        return None;
    }

    // The persisted library cache is owned by the FS watcher / explicit
    // rescans; we don't pollute it with thin shells from ad-hoc slideshow
    // folders. Subsequent calls fall back to spawn_blocking the same way,
    // which still costs nothing while the lock is dropped.
    Some(scanned)
}

/// Initialise the slideshow picker for `profile_name` if it isn't already set.
fn init_picker_if_needed(state: &mut DaemonState, profile_name: &str) {
    if state.slideshow_picker.is_some() {
        return;
    }
    let Some(profile) = state
        .config
        .profiles
        .iter()
        .find(|p| p.name == profile_name)
    else {
        return;
    };
    let slideshow_cfg = match &profile.body {
        ProfileBody::Span(span) => match &span.source {
            SpanSource::Slideshow { config, .. } => config.clone(),
            SpanSource::Single { .. } => return,
        },
        ProfileBody::PerMonitor(_) => return,
    };
    let picker_cfg = profile_to_picker_config(&slideshow_cfg);
    state.slideshow_picker = Some(SlideshowPicker::new(picker_cfg));
}

async fn update_active_profile(state: &Arc<Mutex<DaemonState>>, name: &str) {
    let mut guard = state.lock().await;
    guard.active_profile = Some(name.to_owned());
    guard.last_apply_unix_secs = Some(DaemonState::now_unix_secs());
}

async fn update_timer(state: &Arc<Mutex<DaemonState>>, timer_tx: &watch::Sender<Option<Duration>>) {
    let interval = state.lock().await.active_slideshow_interval();
    let _ = timer_tx.send(interval);
}

fn applied_json(report: &AppliedReport) -> serde_json::Value {
    json!({
        "monitors_set": report.monitors_set,
        "backend": report.backend,
        "elapsed_ms": report.duration.as_millis(),
    })
}

// --- frame I/O ---

/// Hard cap on a single IPC frame body. Requests are tiny JSON objects;
/// anything larger is treated as a hostile or malformed sender.
pub(crate) const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub(crate) async fn read_frame(stream: &mut UnixStream) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame length overflows usize",
        )
    })?;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame length {len} exceeds {MAX_FRAME_BYTES}-byte cap"),
        ));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    Ok(body)
}

pub(crate) async fn write_frame(stream: &mut UnixStream, data: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(data.len()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "response exceeds 4 GiB")
    })?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(data).await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn read_frame_rejects_oversize_length_before_allocating() {
        // Arrange — pair of streams; writer sends a hostile length prefix.
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let oversize = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        writer.write_all(&oversize.to_be_bytes()).await.unwrap();
        // Close the writer so the reader does not block waiting for body bytes.
        drop(writer);

        // Act
        let result = read_frame(&mut reader).await;

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("exceeds"),
            "unexpected error: {err}"
        );
    }
}
