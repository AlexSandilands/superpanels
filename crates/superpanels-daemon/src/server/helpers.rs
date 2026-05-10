//! Shared helpers for the apply / slideshow handlers.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::backends::AppliedReport;
use superpanels_core::config::{ImageSet, ProfileBody, SpanSource};
use superpanels_core::slideshow::SlideshowPicker;
use tokio::sync::{Mutex, watch};
use tracing::error;

use crate::apply::profile_to_picker_config;
use crate::pool::{pool_from_cache, scan_blocking};
use crate::state::DaemonState;

/// Resolve `images` to a list of paths without holding `state` locked across
/// any disk walk. Tries the cached library first; if that misses, runs
/// `scan_folder` in `spawn_blocking` and returns the paths. The persisted
/// library cache is owned by the FS watcher and explicit rescans, so we
/// don't pollute it from ad-hoc slideshow folders.
pub(super) async fn resolve_pool(
    state: &Arc<Mutex<DaemonState>>,
    images: &ImageSet,
) -> Option<Vec<PathBuf>> {
    let cached = {
        let guard = state.lock().await;
        pool_from_cache(images, &guard.library)
    };
    if let Some(pool) = cached {
        if !pool.is_empty() {
            return Some(pool);
        }
    }

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
    Some(scanned)
}

pub(super) fn init_picker_if_needed(state: &mut DaemonState, profile_name: &str) {
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

pub(super) async fn update_active_profile(state: &Arc<Mutex<DaemonState>>, name: &str) {
    let mut guard = state.lock().await;
    guard.active_profile = Some(name.to_owned());
    guard.last_apply_unix_secs = Some(DaemonState::now_unix_secs());
    let now = superpanels_core::config::now_timestamp();
    let mut touched = false;
    if let Some(profile) = guard.config.profiles.iter_mut().find(|p| p.name == name) {
        profile.last_applied_at = Some(now);
        touched = true;
    }
    if touched {
        let path = match guard.config_save_path() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "no config path; skipping last_applied_at persist");
                return;
            }
        };
        if let Err(e) = guard.config.save_to(&path) {
            tracing::warn!(error = %e, "could not persist last_applied_at");
        }
    }
}

pub(super) async fn update_timer(
    state: &Arc<Mutex<DaemonState>>,
    timer_tx: &watch::Sender<Option<Duration>>,
) {
    let interval = state.lock().await.active_slideshow_interval();
    let _ = timer_tx.send(interval);
}

pub(super) fn applied_json(report: &AppliedReport) -> serde_json::Value {
    json!({
        "monitors_set": report.monitors_set,
        "backend": report.backend,
        "elapsed_ms": report.duration.as_millis(),
    })
}
