//! Profile-body shape for `[[profile]]` blocks (`SPEC.md` §3.4).
//!
//! Split into its own file so the top-level `config.rs` stays focused on
//! Config / load / save, and the body schema (which includes the nested
//! span/per-monitor enum dance) lives with its own tests where useful.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::display::MonitorRef;
use crate::layout::FitMode;

/// `body` of a [`super::Profile`]: span-everything vs pin-per-monitor.
///
/// The two-enum dance — `body` for span/per-monitor, `source` (within
/// `Span`) for single/slideshow — makes illegal combos like
/// "per-monitor + slideshow" unrepresentable (`SPEC.md` §3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileBody {
    /// One image at a time, spanned across all monitors.
    Span(SpanProfile),
    /// One image pinned per monitor.
    PerMonitor(PerMonitorProfile),
}

/// Inner body of [`ProfileBody::Span`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanProfile {
    /// Single file, slideshow folder, or playlist.
    pub source: SpanSource,
    /// Fit mode used by `compute_crop_specs`.
    #[serde(default)]
    pub fit: FitMode,
    /// Image-position offset in canvas px (`SPEC.md` §8.3).
    #[serde(default)]
    pub offset: [i32; 2],
}

/// Source of imagery for a [`SpanProfile`]: single file or rotating set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpanSource {
    /// One file on disk.
    Single {
        /// Image path (typically tilde-expanded by the caller before use).
        path: PathBuf,
    },
    /// Rotating set, driven by [`SlideshowConfig`].
    Slideshow {
        /// The pool of images to draw from.
        images: ImageSet,
        /// Rotation parameters.
        config: SlideshowConfig,
    },
}

/// Pool of images a [`SpanSource::Slideshow`] draws from (`SPEC.md` §3.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSet {
    /// Folder scan; respects `recursive` and the optional filters.
    Folder {
        /// Folder path.
        path: PathBuf,
        /// Recurse into subdirectories.
        #[serde(default)]
        recursive: bool,
    },
    /// Hand-curated list of files.
    Playlist {
        /// Ordered list of image paths.
        paths: Vec<PathBuf>,
    },
}

/// Slideshow rotation parameters.
///
/// Subset of `SPEC.md` §9.2 sufficient for §1.6's config round-trip; the
/// full set (history, `on_start`, …) lives here too so the schema is stable
/// from day one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideshowConfig {
    /// Time between automatic advances, serialised as `interval_secs`.
    #[serde(rename = "interval_secs", with = "duration_secs")]
    pub interval: Duration,
    /// Picker order.
    #[serde(default)]
    pub sort: SlideshowSort,
    /// Suppress the most-recent N images from re-selection.
    #[serde(default = "default_recent_history")]
    pub recent_history_size: usize,
    /// Behaviour at daemon start: resume, pick fresh, or first-in-list.
    #[serde(default)]
    pub on_start: SlideshowStart,
    /// Pause the timer when the user advances manually.
    #[serde(default)]
    pub pause_when_active: bool,
    /// Treat a vanished file as a skip rather than an error.
    #[serde(default = "default_skip_on_unavailable")]
    pub skip_on_unavailable: bool,
}

fn default_recent_history() -> usize {
    10
}

fn default_skip_on_unavailable() -> bool {
    true
}

/// Picker order for slideshow advances.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideshowSort {
    /// Random pick, suppressed by `recent_history_size`.
    #[default]
    Shuffle,
    /// File-name ascending.
    Alphabetical,
    /// Mtime ascending.
    DateAsc,
    /// Mtime descending.
    DateDesc,
    /// Least-recently-shown first.
    LastShownAsc,
}

/// Behaviour when the slideshow starts (daemon boot, profile switch).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideshowStart {
    /// Pick up where the previous session left off.
    #[default]
    Resume,
    /// Pick a fresh random image.
    NewRandom,
    /// Show the first image in the picker order.
    First,
}

/// Inner body of [`ProfileBody::PerMonitor`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerMonitorProfile {
    /// Per-monitor image pin. Layout step resolves `MonitorRef` → live
    /// `MonitorId` at apply time.
    pub assignments: Vec<PerMonitorAssignment>,
    /// Fit mode applied per-monitor.
    #[serde(default)]
    pub fit: FitMode,
}

/// One row of [`PerMonitorProfile::assignments`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerMonitorAssignment {
    /// Persistent monitor reference.
    pub monitor: MonitorRef,
    /// Image to set on that monitor.
    pub path: PathBuf,
}

/// Time-of-day trigger (`SPEC.md` §9.3). Subset of variants; richer
/// validation deferred to Phase 2.7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Schedule {
    /// `HH:MM` daily trigger that switches to `profile`.
    Daily {
        /// Local-time hour (0..24).
        hour: u8,
        /// Local-time minute (0..60).
        minute: u8,
        /// Profile name to apply.
        profile: String,
    },
    /// Sunset offset; lat/long supplied separately at evaluator time.
    Sunset {
        /// Offset, in minutes, applied to civil sunset.
        offset_minutes: i32,
        /// Profile name to apply.
        profile: String,
    },
    /// Cron-style expression evaluated on a 60-second tick.
    Cron {
        /// Five-field cron string.
        expr: String,
    },
}

mod duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(d: &Duration, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u64(d.as_secs())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(de)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serde errors
mod tests {
    use super::*;
    use crate::layout::FitMode;
    use std::path::PathBuf;

    #[test]
    fn span_single_round_trips_through_toml() {
        // Arrange
        let body = ProfileBody::Span(SpanProfile {
            source: SpanSource::Single {
                path: PathBuf::from("/walls/x.jpg"),
            },
            fit: FitMode::Fill,
            offset: [10, 0],
        });

        // Act
        let s = toml::to_string(&body).unwrap();
        let back: ProfileBody = toml::from_str(&s).unwrap();

        // Assert
        assert_eq!(back, body);
    }

    #[test]
    fn slideshow_config_round_trips_seconds() {
        // Arrange
        let body = ProfileBody::Span(SpanProfile {
            source: SpanSource::Slideshow {
                images: ImageSet::Folder {
                    path: PathBuf::from("/walls"),
                    recursive: true,
                },
                config: SlideshowConfig {
                    interval: std::time::Duration::from_secs(1800),
                    sort: SlideshowSort::Shuffle,
                    recent_history_size: 10,
                    on_start: SlideshowStart::Resume,
                    pause_when_active: false,
                    skip_on_unavailable: true,
                },
            },
            fit: FitMode::Fill,
            offset: [0, 0],
        });

        // Act
        let s = toml::to_string(&body).unwrap();
        let back: ProfileBody = toml::from_str(&s).unwrap();

        // Assert
        assert_eq!(back, body);
    }
}
