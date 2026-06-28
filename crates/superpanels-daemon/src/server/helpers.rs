//! Shared helpers for the apply / slideshow handlers.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use superpanels_core::backends::AppliedReport;
use superpanels_core::config::ProfileBody;
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
        ProfileBody::Slideshow(slideshow) => slideshow.source.config.clone(),
        ProfileBody::Standard(_) | ProfileBody::PerMonitor(_) => return,
    };
    let picker_cfg = profile_to_picker_config(&slideshow_cfg);
    state.slideshow_picker = Some(SlideshowPicker::new(picker_cfg));
}

pub(super) async fn update_active_profile(
    state: &Arc<Mutex<DaemonState>>,
    name: &str,
    backend: &str,
) {
    let mut guard = state.lock().await;
    guard.active_profile = Some(name.to_owned());
    guard.last_apply_backend = Some(backend.to_owned());
    guard.last_apply_unix_secs = Some(DaemonState::now_unix_secs());
    persist_resume(&guard);
    // A leftover picker would keep `current_state` reporting a slideshow
    // (with stale pool/counter data) after switching to a non-slideshow.
    let is_slideshow = guard
        .config
        .profiles
        .iter()
        .any(|p| p.name == name && matches!(&p.body, ProfileBody::Slideshow(_)));
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

/// Best-effort write of the resume file so a daemon restart lands back on the
/// current profile. Failure only warns — the apply itself already succeeded.
pub(super) fn persist_resume(state: &DaemonState) {
    let (Some(path), Some(active)) = (
        state.resume_path.as_deref(),
        state.active_profile.as_deref(),
    ) else {
        return;
    };
    let resume = superpanels_core::resume::ResumeState {
        active_profile: active.to_owned(),
        last_apply_backend: state.last_apply_backend.clone(),
        last_apply_unix_secs: state.last_apply_unix_secs,
    };
    if let Err(e) = superpanels_core::resume::save(&resume, path) {
        tracing::warn!(error = %e, "could not persist resume state");
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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use superpanels_core::config::Config;
    use superpanels_core::resume;
    use tempfile::tempdir;

    #[tokio::test]
    async fn update_active_profile_persists_resume_state() {
        let dir = tempdir().unwrap();
        let resume_path = dir.path().join("resume-state.json");
        let mut ds = DaemonState::for_tests(Config::default());
        ds.resume_path = Some(resume_path.clone());
        let state = Arc::new(Mutex::new(ds));

        update_active_profile(&state, "Lofi", "kde").await;

        let saved = resume::load(&resume_path).unwrap().unwrap();
        assert_eq!(saved.active_profile, "Lofi");
        assert_eq!(saved.last_apply_backend.as_deref(), Some("kde"));
        assert!(saved.last_apply_unix_secs.is_some());
    }

    #[tokio::test]
    async fn update_active_profile_without_resume_path_skips_persist() {
        let state = Arc::new(Mutex::new(DaemonState::for_tests(Config::default())));
        update_active_profile(&state, "Lofi", "kde").await;
        let guard = state.lock().await;
        assert_eq!(guard.active_profile.as_deref(), Some("Lofi"));
        assert_eq!(guard.last_apply_backend.as_deref(), Some("kde"));
    }
}
