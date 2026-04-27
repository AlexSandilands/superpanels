//! Slideshow timer task: fires `slideshow_tick` when the active profile has a
//! slideshow and the timer is not paused.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use superpanels_core::config::{ProfileBody, SpanSource};
use superpanels_core::library::scan_folder;
use tokio::sync::{Mutex, watch};
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, warn};

use crate::apply::run_span_apply;
use crate::state::DaemonState;

/// Runs forever. Watches `ctrl_rx` for the active slideshow interval.
/// `None` = no slideshow active; `Some(d)` = advance every `d`.
pub(crate) async fn run_timer(
    state: Arc<Mutex<DaemonState>>,
    mut ctrl_rx: watch::Receiver<Option<Duration>>,
) {
    loop {
        // Borrow the current control value.
        let interval = *ctrl_rx.borrow_and_update();
        match interval {
            None => {
                // No active slideshow — wait for a change.
                if ctrl_rx.changed().await.is_err() {
                    return; // sender was dropped; daemon is shutting down
                }
            }
            Some(interval) => {
                let mut tick = tokio::time::interval(interval);
                tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
                // Skip the immediate first tick so we don't fire on activation.
                tick.tick().await;

                loop {
                    tokio::select! {
                        _ = tick.tick() => {
                            // Re-read interval in case it changed while waiting.
                            let current = *ctrl_rx.borrow();
                            if current != Some(interval) {
                                break; // profile or interval changed; restart
                            }
                            slideshow_tick(Arc::clone(&state)).await;
                        }
                        changed = ctrl_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            break; // control changed; re-enter outer loop
                        }
                    }
                }
            }
        }
    }
}

/// Pick and apply the next slideshow image for the active profile. Skips if
/// the slideshow is paused or the pool is empty.
async fn slideshow_tick(state: Arc<Mutex<DaemonState>>) {
    // Collect everything needed from state under lock, then release.
    let (profile, monitors, backend_kind, custom_cmd, image_path) = {
        let mut guard = state.lock().await;

        if guard
            .slideshow_picker
            .as_ref()
            .is_none_or(|p| p.state().paused)
        {
            debug!("slideshow tick skipped: paused or no picker");
            return;
        }

        let Some(name) = guard.active_profile.clone() else {
            return;
        };
        let Some(profile) = guard
            .config
            .profiles
            .iter()
            .find(|p| p.name == name)
            .cloned()
        else {
            return;
        };
        let images = match &profile.body {
            ProfileBody::Span(span) => match &span.source {
                SpanSource::Slideshow { images, .. } => images.clone(),
                SpanSource::Single { .. } => return,
            },
            ProfileBody::PerMonitor(_) => return,
        };

        let pool: Vec<PathBuf> = match &images {
            superpanels_core::config::ImageSet::Folder { path, recursive } => {
                scan_folder(path, *recursive, |_| {})
                    .into_iter()
                    .map(|e| e.path)
                    .collect()
            }
            superpanels_core::config::ImageSet::Playlist { paths } => paths.clone(),
        };

        let backend_kind = profile
            .backend_override
            .unwrap_or(guard.config.backend.prefer);
        let custom_cmd = guard.config.backend.custom_command.clone();
        let monitors = guard.monitors.clone();

        let path = guard
            .slideshow_picker
            .as_mut()
            .and_then(|p| p.next(&pool).ok());

        (profile, monitors, backend_kind, custom_cmd, path)
    };

    let Some(image_path) = image_path else {
        warn!("slideshow tick: pool is empty or all images in history");
        return;
    };

    info!(image = %image_path.display(), "slideshow tick: applying next image");

    let result = tokio::task::spawn_blocking(move || {
        run_span_apply(&image_path, &monitors, &profile, backend_kind, &custom_cmd)
    })
    .await;

    match result {
        Ok(Ok(report)) => {
            let mut guard = state.lock().await;
            guard.last_apply_unix_secs = Some(DaemonState::now_unix_secs());
            debug!(
                monitors = report.monitors_set,
                elapsed_ms = report.duration.as_millis(),
                "slideshow tick applied"
            );
        }
        Ok(Err(e)) => warn!(error = %e, "slideshow tick apply failed"),
        Err(e) => warn!(error = %e, "slideshow tick task panicked"),
    }
}
