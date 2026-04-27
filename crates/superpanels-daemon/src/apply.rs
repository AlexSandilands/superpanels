//! Apply pipeline: detect → crop → render → backend (`SPEC.md` §10).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use superpanels_core::backends::{AppliedReport, WallpaperBackend, detect_backend};
use superpanels_core::config::{
    BackendKind, PerMonitorAssignment, Profile, ProfileBody,
    SlideshowConfig as ProfileSlideshowConfig, SlideshowSort as ProfileSlideshowSort,
    SlideshowStart as ProfileSlideshowStart,
};
use superpanels_core::display::{Monitor, MonitorRef};
use superpanels_core::image::{
    FitMode as ImageFitMode, clear_temp_dir, crop, load, rotate, save_temp, scale_to_fit,
};
use superpanels_core::layout::{FitMode, compute_crop_specs};
use superpanels_core::slideshow::{
    SlideshowConfig as PickerSlideshowConfig, SlideshowSort as PickerSort,
    SlideshowStart as PickerStart,
};
use tracing::{debug, info};

/// Convert the config-layer [`ProfileSlideshowConfig`] to the runtime
/// [`PickerSlideshowConfig`]. The two types are structurally identical but live
/// in separate modules to decouple serialization format from picker logic.
pub(crate) fn profile_to_picker_config(cfg: &ProfileSlideshowConfig) -> PickerSlideshowConfig {
    PickerSlideshowConfig {
        interval: cfg.interval,
        sort: match cfg.sort {
            ProfileSlideshowSort::Shuffle => PickerSort::Shuffle,
            ProfileSlideshowSort::Alphabetical => PickerSort::Alphabetical,
            ProfileSlideshowSort::DateAsc => PickerSort::DateAsc,
            ProfileSlideshowSort::DateDesc => PickerSort::DateDesc,
            ProfileSlideshowSort::LastShownAsc => PickerSort::LastShownAsc,
        },
        recent_history_size: cfg.recent_history_size,
        on_start: match cfg.on_start {
            ProfileSlideshowStart::Resume => PickerStart::Resume,
            ProfileSlideshowStart::NewRandom => PickerStart::NewRandom,
            ProfileSlideshowStart::First => PickerStart::First,
        },
        pause_when_active: cfg.pause_when_active,
        skip_on_unavailable: cfg.skip_on_unavailable,
    }
}

/// Synchronous image processing + backend apply for a single-span wallpaper.
/// Designed to run inside `tokio::task::spawn_blocking`.
pub(crate) fn run_span_apply(
    image_path: &Path,
    monitors: &[Monitor],
    profile: &Profile,
    backend_kind: BackendKind,
    custom_cmd: &str,
) -> Result<AppliedReport> {
    let bezels = profile.bezels;
    let fit = match &profile.body {
        ProfileBody::Span(s) => s.fit,
        ProfileBody::PerMonitor(pm) => pm.fit,
    };

    let source =
        load(image_path).with_context(|| format!("loading image {}", image_path.display()))?;
    let image_size = (source.width(), source.height());

    let specs =
        compute_crop_specs(monitors, &bezels, fit, image_size).context("computing crop specs")?;

    let backend: Box<dyn WallpaperBackend> = detect_backend(backend_kind, custom_cmd);
    if backend.availability() != superpanels_core::Availability::Available
        && backend_kind != BackendKind::Auto
    {
        // For non-Auto, detect_backend returns the requested kind regardless;
        // apply() will surface the real error.
        debug!(kind = ?backend_kind, "using pinned backend despite unavailability");
    }

    clear_temp_dir().context("clearing temp dir")?;
    let token = apply_token();
    let mut assignments: Vec<(MonitorRef, PathBuf)> = Vec::with_capacity(specs.len());
    for spec in &specs {
        let monitor = monitors
            .iter()
            .find(|m| m.id == spec.monitor_id)
            .ok_or_else(|| {
                anyhow::anyhow!("crop spec references unknown monitor {:?}", spec.monitor_id)
            })?;
        let cropped = crop(&source, spec.src_rect).context("cropping image")?;
        let resized = scale_to_fit(&cropped, spec.dst_size, ImageFitMode::Stretch);
        let rotated = rotate(&resized, spec.rotation);
        let safe = sanitise_filename(&monitor.name);
        let filename = format!("{safe}-{token}.png");
        let path = save_temp(&rotated, &filename).context("saving temp slice")?;
        debug!(monitor = %monitor.name, file = %path.display(), "wrote temp slice");
        assignments.push((to_monitor_ref(monitor), path));
    }

    let report = backend.apply(&assignments).context("backend apply")?;
    info!(
        backend = report.backend,
        monitors = report.monitors_set,
        elapsed_ms = report.duration.as_millis(),
        "apply complete"
    );
    Ok(report)
}

/// Apply a per-monitor profile. Each monitor gets its configured image.
pub(crate) fn run_per_monitor_apply(
    assignments: &[PerMonitorAssignment],
    monitors: &[Monitor],
    fit: FitMode,
    backend_kind: BackendKind,
    custom_cmd: &str,
) -> Result<AppliedReport> {
    let backend: Box<dyn WallpaperBackend> = detect_backend(backend_kind, custom_cmd);

    clear_temp_dir().context("clearing temp dir")?;
    let token = apply_token();
    let mut backend_assignments: Vec<(MonitorRef, PathBuf)> = Vec::with_capacity(assignments.len());

    for assignment in assignments {
        // Resolve MonitorRef to a live Monitor for sizing.
        let monitor = monitors.iter().find(|m| {
            m.stable_id
                .as_deref()
                .is_some_and(|id| id == assignment.monitor.stable_id)
                || m.name == assignment.monitor.name
        });
        if let Some(monitor) = monitor {
            let source = load(&assignment.path)
                .with_context(|| format!("loading {}", assignment.path.display()))?;
            let resized = scale_to_fit(&source, monitor.resolution, layout_fit_to_image_fit(fit));
            let safe = sanitise_filename(&monitor.name);
            let path = save_temp(&resized, &format!("{safe}-{token}.png"))
                .context("saving per-monitor temp file")?;
            backend_assignments.push((to_monitor_ref(monitor), path));
        } else {
            // Monitor not found in current layout — emit warning but continue.
            tracing::warn!(
                monitor = %assignment.monitor.name,
                "monitor not found in current layout; skipping"
            );
        }
    }

    let report = backend
        .apply(&backend_assignments)
        .context("backend apply")?;
    Ok(report)
}

fn to_monitor_ref(m: &Monitor) -> MonitorRef {
    MonitorRef {
        stable_id: m.stable_id.clone().unwrap_or_else(|| m.name.clone()),
        name: m.name.clone(),
    }
}

fn sanitise_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Nanosecond timestamp used as a per-apply cache-buster for compositor image caches.
fn apply_token() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn layout_fit_to_image_fit(f: FitMode) -> ImageFitMode {
    match f {
        FitMode::Fill => ImageFitMode::Fill,
        FitMode::Fit => ImageFitMode::Fit,
        FitMode::Stretch => ImageFitMode::Stretch,
        FitMode::Center => ImageFitMode::Center,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected errors
mod tests {
    use super::*;
    use std::time::Duration;

    fn cfg_from_parts(
        sort: ProfileSlideshowSort,
        on_start: ProfileSlideshowStart,
    ) -> ProfileSlideshowConfig {
        ProfileSlideshowConfig {
            interval: Duration::from_secs(1800),
            sort,
            recent_history_size: 10,
            on_start,
            pause_when_active: false,
            skip_on_unavailable: true,
        }
    }

    #[test]
    fn sort_variants_convert_correctly() {
        // Arrange + Act
        let variants = [
            ProfileSlideshowSort::Shuffle,
            ProfileSlideshowSort::Alphabetical,
            ProfileSlideshowSort::DateAsc,
            ProfileSlideshowSort::DateDesc,
            ProfileSlideshowSort::LastShownAsc,
        ];
        let expected = [
            PickerSort::Shuffle,
            PickerSort::Alphabetical,
            PickerSort::DateAsc,
            PickerSort::DateDesc,
            PickerSort::LastShownAsc,
        ];

        // Assert
        for (profile_sort, picker_sort) in variants.into_iter().zip(expected) {
            let cfg = cfg_from_parts(profile_sort, ProfileSlideshowStart::Resume);
            let converted = profile_to_picker_config(&cfg);
            assert_eq!(converted.sort, picker_sort);
        }
    }

    #[test]
    fn on_start_variants_convert_correctly() {
        // Arrange + Act + Assert
        let pairs = [
            (ProfileSlideshowStart::Resume, PickerStart::Resume),
            (ProfileSlideshowStart::NewRandom, PickerStart::NewRandom),
            (ProfileSlideshowStart::First, PickerStart::First),
        ];
        for (profile_start, picker_start) in pairs {
            let cfg = cfg_from_parts(ProfileSlideshowSort::Shuffle, profile_start);
            let converted = profile_to_picker_config(&cfg);
            assert_eq!(converted.on_start, picker_start);
        }
    }

    #[test]
    fn interval_is_preserved() {
        let cfg = cfg_from_parts(ProfileSlideshowSort::Shuffle, ProfileSlideshowStart::Resume);
        let converted = profile_to_picker_config(&cfg);
        assert_eq!(converted.interval, Duration::from_secs(1800));
    }

    #[test]
    fn sanitise_filename_removes_path_separators() {
        assert_eq!(
            sanitise_filename("DP-1/../etc/passwd"),
            "DP-1_.._etc_passwd"
        );
    }
}
