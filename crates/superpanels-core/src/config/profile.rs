//! Profile-body shape for `[[profile]]` blocks.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::display::MonitorRef;
use crate::layout::{FitMode, ImageRectMm};

/// `body` of a [`super::Profile`]. The `body`/`source` two-enum split makes
/// "per-monitor + slideshow" unrepresentable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileBody {
    Span(SpanProfile),
    PerMonitor(PerMonitorProfile),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct SpanProfile {
    pub source: SpanSource,
    /// The image's rectangle in canvas mm-space — the canvas is the source
    /// of truth: monitors crop whatever they overlap with this rectangle.
    pub image_rect_mm: ImageRectMm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
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

/// Mixed slideshow pool: any number of live folders and hand-picked images.
/// Deserialization also accepts the pre-1.0 single-variant forms
/// (`type = "folder"` / `type = "playlist"`) and lifts them into `sources`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct ImageSet {
    pub sources: Vec<ImageSource>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Folder {
        path: PathBuf,
        #[serde(default)]
        recursive: bool,
    },
    Image {
        path: PathBuf,
    },
}

impl ImageSet {
    #[must_use]
    pub fn from_folder(path: PathBuf, recursive: bool) -> Self {
        Self {
            sources: vec![ImageSource::Folder { path, recursive }],
        }
    }

    #[must_use]
    pub fn from_images(paths: Vec<PathBuf>) -> Self {
        Self {
            sources: paths
                .into_iter()
                .map(|path| ImageSource::Image { path })
                .collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl<'de> Deserialize<'de> for ImageSet {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Compat {
            Current { sources: Vec<ImageSource> },
            Legacy(LegacySet),
        }
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum LegacySet {
            Folder {
                path: PathBuf,
                #[serde(default)]
                recursive: bool,
            },
            Playlist {
                paths: Vec<PathBuf>,
            },
        }
        Ok(match Compat::deserialize(de)? {
            Compat::Current { sources } => Self { sources },
            Compat::Legacy(LegacySet::Folder { path, recursive }) => {
                Self::from_folder(path, recursive)
            }
            Compat::Legacy(LegacySet::Playlist { paths }) => Self::from_images(paths),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct SlideshowConfig {
    #[serde(rename = "interval_secs", with = "duration_secs")]
    #[ts(type = "number")]
    pub interval: Duration,
    #[serde(default)]
    pub sort: SlideshowSort,
    #[serde(default = "default_recent_history")]
    pub recent_history_size: usize,
    #[serde(default)]
    pub on_start: SlideshowStart,
    #[serde(default)]
    pub pause_when_active: bool,
    #[serde(default = "default_skip_on_unavailable")]
    pub skip_on_unavailable: bool,
}

fn default_recent_history() -> usize {
    10
}

fn default_skip_on_unavailable() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(rename_all = "snake_case")]
pub enum SlideshowSort {
    #[default]
    Shuffle,
    Alphabetical,
    DateAsc,
    DateDesc,
    LastShownAsc,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(rename_all = "snake_case")]
pub enum SlideshowStart {
    #[default]
    Resume,
    NewRandom,
    First,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct PerMonitorProfile {
    pub assignments: Vec<PerMonitorAssignment>,
    #[serde(default)]
    pub fit: FitMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct PerMonitorAssignment {
    pub monitor: MonitorRef,
    pub path: PathBuf,
}

/// Type alias rather than a wrapper struct: `chrono::DateTime<Utc>`
/// already serialises to / from RFC-3339 strings, and ts-rs's
/// `chrono-impl` feature gives it a TypeScript representation. A
/// tuple-struct wrapper would force `#[serde(transparent)]`, which
/// ts-rs's derive macro emits a parse-warning for.
pub type ProfileTimestamp = DateTime<Utc>;

/// Free function for "now" — used as a `#[serde(default = ...)]` and
/// from any caller that previously held `now_timestamp()`.
#[must_use]
pub fn now_timestamp() -> ProfileTimestamp {
    Utc::now()
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
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;

    #[test]
    fn image_set_round_trips_mixed_sources_through_json() {
        let set = ImageSet {
            sources: vec![
                ImageSource::Folder {
                    path: PathBuf::from("/walls"),
                    recursive: true,
                },
                ImageSource::Image {
                    path: PathBuf::from("/pick/a.png"),
                },
            ],
        };
        let json = serde_json::to_string(&set).unwrap();
        let back: ImageSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn legacy_folder_form_deserializes_into_one_folder_source() {
        let json = r#"{"type":"folder","path":"/walls","recursive":true}"#;
        let set: ImageSet = serde_json::from_str(json).unwrap();
        assert_eq!(set, ImageSet::from_folder(PathBuf::from("/walls"), true));
    }

    #[test]
    fn legacy_playlist_form_deserializes_into_image_sources() {
        let json = r#"{"type":"playlist","paths":["/a.png","/b.png"]}"#;
        let set: ImageSet = serde_json::from_str(json).unwrap();
        assert_eq!(
            set,
            ImageSet::from_images(vec![PathBuf::from("/a.png"), PathBuf::from("/b.png")])
        );
    }

    #[test]
    fn legacy_folder_form_deserializes_from_toml() {
        // Profiles are persisted as TOML; the compat path must hold there too.
        let toml = "type = \"folder\"\npath = \"/walls\"\n";
        let set: ImageSet = toml::from_str(toml).unwrap();
        assert_eq!(set, ImageSet::from_folder(PathBuf::from("/walls"), false));
    }
}
