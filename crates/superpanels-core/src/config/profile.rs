//! Profile-body shape for `[[profile]]` blocks (`SPEC.md` §3.4).

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::display::MonitorRef;
use crate::layout::FitMode;

/// `body` of a [`super::Profile`]. The `body`/`source` two-enum split makes
/// "per-monitor + slideshow" unrepresentable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileBody {
    Span(SpanProfile),
    PerMonitor(PerMonitorProfile),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanProfile {
    pub source: SpanSource,
    #[serde(default)]
    pub fit: FitMode,
    /// Image-position offset in canvas px (`SPEC.md` §8.3).
    #[serde(default)]
    pub offset: [i32; 2],
    /// Explicit image rectangle in canvas px (`docs/spec/12-gui.md` §12.3).
    /// `None` defers to `fit`; `Some([w, h])` pins the GUI's free transform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_size_px: Option<[u32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpanSource {
    Single {
        path: PathBuf,
    },
    Slideshow {
        images: ImageSet,
        config: SlideshowConfig,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSet {
    Folder {
        path: PathBuf,
        #[serde(default)]
        recursive: bool,
    },
    Playlist {
        paths: Vec<PathBuf>,
    },
}

/// Slideshow rotation parameters (`SPEC.md` §9.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideshowConfig {
    #[serde(rename = "interval_secs", with = "duration_secs")]
    pub interval: Duration,
    #[serde(default)]
    pub sort: SlideshowSort,
    /// Suppress the most-recent N images from re-selection.
    #[serde(default = "default_recent_history")]
    pub recent_history_size: usize,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideshowSort {
    #[default]
    Shuffle,
    Alphabetical,
    DateAsc,
    DateDesc,
    LastShownAsc,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideshowStart {
    #[default]
    Resume,
    NewRandom,
    First,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerMonitorProfile {
    /// Layout step resolves `MonitorRef` → live `MonitorId` at apply time.
    pub assignments: Vec<PerMonitorAssignment>,
    #[serde(default)]
    pub fit: FitMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerMonitorAssignment {
    pub monitor: MonitorRef,
    pub path: PathBuf,
}

/// Time-of-day trigger (`SPEC.md` §9.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Schedule {
    Daily {
        hour: u8,
        minute: u8,
        profile: String,
    },
    Sunset {
        offset_minutes: i32,
        profile: String,
    },
    Cron {
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
#[allow(clippy::panic)] // reason: panic on unexpected enum shape is the test failure
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
            image_size_px: None,
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
            image_size_px: None,
        });

        // Act
        let s = toml::to_string(&body).unwrap();
        let back: ProfileBody = toml::from_str(&s).unwrap();

        // Assert
        assert_eq!(back, body);
    }

    #[test]
    fn span_profile_image_size_px_round_trips_through_toml() {
        // Arrange
        let body = ProfileBody::Span(SpanProfile {
            source: SpanSource::Single {
                path: PathBuf::from("/walls/x.jpg"),
            },
            fit: FitMode::Fill,
            offset: [10, -20],
            image_size_px: Some([3840, 2160]),
        });

        // Act
        let s = toml::to_string(&body).unwrap();
        let back: ProfileBody = toml::from_str(&s).unwrap();

        // Assert
        assert_eq!(back, body);
        assert!(
            s.contains("image_size_px"),
            "expected image_size_px to be serialised when Some, TOML was: {s}"
        );
    }

    #[test]
    fn span_profile_image_size_px_skipped_when_none() {
        // Arrange — `skip_serializing_if = Option::is_none` keeps existing
        // configs visually unchanged when the user hasn't touched the GUI's
        // free transform.
        let body = ProfileBody::Span(SpanProfile {
            source: SpanSource::Single {
                path: PathBuf::from("/walls/x.jpg"),
            },
            fit: FitMode::Fill,
            offset: [0, 0],
            image_size_px: None,
        });

        // Act
        let s = toml::to_string(&body).unwrap();

        // Assert
        assert!(
            !s.contains("image_size_px"),
            "expected image_size_px to be omitted when None, TOML was: {s}"
        );
    }

    #[test]
    fn span_profile_legacy_toml_without_image_size_px_loads_with_none() {
        // Arrange — a Phase 4a profile block with no image_size_px.
        let toml_text = r#"
type = "span"
fit = "fill"
offset = [0, 0]

[source]
type = "single"
path = "/walls/x.jpg"
"#;

        // Act
        let body: ProfileBody = toml::from_str(toml_text).unwrap();

        // Assert
        let ProfileBody::Span(span) = body else {
            panic!("expected Span body");
        };
        assert_eq!(span.image_size_px, None);
    }
}
