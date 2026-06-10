//! Apply pipeline: detect → crop → render → backend.

use std::path::{Path, PathBuf};

use std::collections::HashMap;

use anyhow::{Context, Result};
use superpanels_core::backends::{AppliedReport, WallpaperBackend, detect_backend};
use superpanels_core::config::{
    BackendKind, PerMonitorAssignment, Profile, ProfileBody,
    SlideshowConfig as ProfileSlideshowConfig, SlideshowSort as ProfileSlideshowSort,
    SlideshowStart as ProfileSlideshowStart, SpanSource,
};
use superpanels_core::display::{Monitor, MonitorRef};
use superpanels_core::image::{
    FitMode as ImageFitMode, clear_temp_dir, load, render_slice, save_temp, scale_to_fit,
};
use superpanels_core::layout::{
    FitMode, ImageRectMm, compute_crop_specs, cover_image_rect_for, synthesise_placements,
};
use superpanels_core::schedule::MonitorPlacement;
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
///
/// Every slideshow apply (timer tick, next/prev, profile switch) funnels
/// through here, so this is the one place per-image canvas overrides are
/// resolved — they work daemon-side with the GUI closed.
pub(crate) fn run_span_apply(
    image_path: &Path,
    monitors: &[Monitor],
    profile: &Profile,
    backend_kind: BackendKind,
    custom_cmd: &str,
) -> Result<AppliedReport> {
    let (placements, image_rect_mm) = span_layout_for(profile, image_path);
    run_immediate_span_apply(
        image_path,
        monitors,
        placements,
        image_rect_mm,
        backend_kind,
        custom_cmd,
    )
}

/// Effective placements + image rect for one apply: the image's canvas
/// override when the slideshow carries one, else the profile-level state.
/// Untuned slideshow images get `None` unless the slideshow opted into
/// `uniform_layout` — the profile rect was authored for one specific aspect,
/// so forcing it would squeeze other pictures; the cover-fit fallback in
/// [`run_immediate_span_apply`] slices at the image's own aspect instead,
/// matching the GUI canvas.
fn span_layout_for<'a>(
    profile: &'a Profile,
    image_path: &Path,
) -> (&'a HashMap<String, MonitorPlacement>, Option<ImageRectMm>) {
    match &profile.body {
        ProfileBody::Span(s) => match s.source.override_for(image_path) {
            Some(o) => {
                debug!(image = %image_path.display(), "using per-image canvas override");
                (&o.monitor_state, Some(o.image_rect_mm))
            }
            None => match &s.source {
                SpanSource::Slideshow { uniform_layout, .. } => {
                    let rect = uniform_layout.then_some(s.image_rect_mm);
                    (&profile.monitor_state, rect)
                }
                SpanSource::Single { .. } => (&profile.monitor_state, Some(s.image_rect_mm)),
            },
        },
        ProfileBody::PerMonitor(_) => (&profile.monitor_state, None),
    }
}

/// Crop a single source image across the canvas and push it to a backend.
/// Empty `placements` and missing `image_rect_mm` are filled in via the
/// transient cover-fit fallback used by CLI `set --image` and the daemon's
/// `cmd_set` path.
pub(crate) fn run_immediate_span_apply(
    image_path: &Path,
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement>,
    image_rect_mm: Option<ImageRectMm>,
    backend_kind: BackendKind,
    custom_cmd: &str,
) -> Result<AppliedReport> {
    let source =
        load(image_path).with_context(|| format!("loading image {}", image_path.display()))?;
    let image_size = (source.width(), source.height());

    // Empty placements (CLI `set --image` style) → synthesise from detected
    // positions so the transient apply path still works without a profile.
    let synthesised;
    let resolved_placements: &HashMap<String, MonitorPlacement> = if placements.is_empty() {
        synthesised = synthesise_placements(monitors);
        &synthesised
    } else {
        placements
    };

    // Cover over the *resolved* placements, not detected positions — profile
    // gaps widen the desktop plane and the slice must span that.
    let rect = image_rect_mm
        .unwrap_or_else(|| cover_image_rect_for(monitors, resolved_placements, image_size));
    let specs = compute_crop_specs(monitors, resolved_placements, image_size, rect)
        .context("computing crop specs")?;

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
        // Compose at the canvas/logical (post-rotation) framebuffer dims —
        // the backend writes that file as-is. Pre-rotating to native panel
        // orientation would over-bake: KDE's wallpaper plugin already paints
        // into the rotated framebuffer (memory: KDE wallpaper orientation).
        // Sway/Hyprland/wlroots and feh likewise expect post-rotation files.
        let composed = render_slice(&source, spec).context("composing slice")?;
        let safe = sanitise_filename(&monitor.name);
        let filename = format!("{safe}-{token}.png");
        let path = save_temp(&composed, &filename).context("saving temp slice")?;
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

/// Apply a per-monitor profile. Each monitor gets its configured image,
/// scaled to the monitor's resolution under `fit`.
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
#[allow(clippy::expect_used)] // reason: same
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

    fn slideshow_profile_with_override(image: &Path) -> superpanels_core::Profile {
        use std::path::PathBuf;
        use superpanels_core::TopologyFingerprint;
        use superpanels_core::config::{
            ImageOverride, ImageSet, Profile, ProfileBody, SlideshowSort as Sort,
            SlideshowStart as Start, SpanProfile, SpanSource,
        };

        let mut override_state = HashMap::new();
        override_state.insert(
            "uuid-a".to_owned(),
            MonitorPlacement {
                x_mm: 111.0,
                y_mm: 222.0,
            },
        );
        let overrides = HashMap::from([(
            image.to_path_buf(),
            ImageOverride {
                monitor_state: override_state,
                image_rect_mm: ImageRectMm {
                    x_mm: 9.0,
                    y_mm: 9.0,
                    w_mm: 900.0,
                    h_mm: 300.0,
                },
            },
        )]);
        let now = superpanels_core::config::now_timestamp();
        Profile {
            name: "show".to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Slideshow {
                    images: ImageSet::from_folder(PathBuf::from("/walls"), false),
                    config: ProfileSlideshowConfig {
                        interval: Duration::from_secs(600),
                        sort: Sort::Shuffle,
                        recent_history_size: 10,
                        on_start: Start::Resume,
                        pause_when_active: false,
                        skip_on_unavailable: true,
                    },
                    overrides,
                    uniform_layout: false,
                },
                image_rect_mm: ImageRectMm {
                    x_mm: 0.0,
                    y_mm: 0.0,
                    w_mm: 1800.0,
                    h_mm: 600.0,
                },
            }),
            monitor_state: HashMap::from([(
                "uuid-a".to_owned(),
                MonitorPlacement {
                    x_mm: 0.0,
                    y_mm: 0.0,
                },
            )]),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: None,
        }
    }

    #[test]
    fn span_layout_uses_override_for_tuned_image() {
        let image = Path::new("/walls/tuned.png");
        let profile = slideshow_profile_with_override(image);

        let (placements, rect) = span_layout_for(&profile, image);

        let p = placements.get("uuid-a").expect("override placement");
        assert!((p.x_mm - 111.0).abs() < f32::EPSILON);
        let rect = rect.expect("span profiles always carry a rect");
        assert!((rect.w_mm - 900.0).abs() < f32::EPSILON);
    }

    #[test]
    fn span_layout_untuned_slideshow_image_gets_cover_fallback() {
        let image = Path::new("/walls/tuned.png");
        let profile = slideshow_profile_with_override(image);

        let (placements, rect) = span_layout_for(&profile, Path::new("/walls/other.png"));

        let p = placements.get("uuid-a").expect("profile placement");
        assert!(p.x_mm.abs() < f32::EPSILON);
        // No rect: the profile rect was authored for another image's aspect;
        // run_immediate_span_apply cover-fits this image instead.
        assert!(rect.is_none());
    }

    #[test]
    fn span_layout_uniform_slideshow_uses_profile_rect_for_untuned_image() {
        let image = Path::new("/walls/tuned.png");
        let mut profile = slideshow_profile_with_override(image);
        if let ProfileBody::Span(s) = &mut profile.body
            && let SpanSource::Slideshow { uniform_layout, .. } = &mut s.source
        {
            *uniform_layout = true;
        }

        let (_, rect) = span_layout_for(&profile, Path::new("/walls/other.png"));
        let rect = rect.expect("uniform slideshows apply the profile rect");
        assert!((rect.w_mm - 1800.0).abs() < f32::EPSILON);

        // A hand-tuned image still wins over the uniform layout.
        let (_, rect) = span_layout_for(&profile, image);
        let rect = rect.expect("override rect");
        assert!((rect.w_mm - 900.0).abs() < f32::EPSILON);
    }

    #[test]
    fn span_layout_single_source_keeps_profile_rect() {
        use superpanels_core::TopologyFingerprint;
        use superpanels_core::config::{Profile, ProfileBody, SpanProfile, SpanSource};

        let now = superpanels_core::config::now_timestamp();
        let profile = Profile {
            name: "single".to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Single {
                    path: std::path::PathBuf::from("/walls/a.png"),
                },
                image_rect_mm: ImageRectMm {
                    x_mm: 0.0,
                    y_mm: 0.0,
                    w_mm: 1800.0,
                    h_mm: 600.0,
                },
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: None,
        };

        let (_, rect) = span_layout_for(&profile, Path::new("/walls/a.png"));

        let rect = rect.expect("single sources carry their authored rect");
        assert!((rect.w_mm - 1800.0).abs() < f32::EPSILON);
    }
}
