//! Shared helpers for the apply / slideshow handlers.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::backends::AppliedReport;
use superpanels_core::config::{ProfileBody, SpanSource};
use superpanels_core::slideshow::SlideshowPicker;
use tokio::sync::{Mutex, watch};

use crate::apply::profile_to_picker_config;
use crate::state::DaemonState;

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
    // A leftover picker would keep `current_state` reporting a slideshow
    // (with stale pool/counter data) after switching to a non-slideshow.
    let is_slideshow = guard.config.profiles.iter().any(|p| {
        p.name == name
            && matches!(
                &p.body,
                ProfileBody::Span(span) if matches!(span.source, SpanSource::Slideshow { .. })
            )
    });
    if !is_slideshow {
        guard.clear_slideshow_runtime();
    }
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

/// Sync the timer with the active profile's interval. The countdown only
/// restarts when the interval actually changed — image-set or sort edits on
/// a running slideshow must not push back the next advance.
pub(super) async fn update_timer(
    state: &Arc<Mutex<DaemonState>>,
    timer_tx: &watch::Sender<Option<Duration>>,
) {
    let interval = state.lock().await.active_slideshow_interval();
    timer_tx.send_if_modified(|current| {
        if *current == interval {
            false
        } else {
            *current = interval;
            true
        }
    });
}

/// Re-arm the timer unconditionally, restarting the countdown even when the
/// interval is unchanged — applies and manual advances reset the clock.
pub(super) async fn restart_timer(
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
