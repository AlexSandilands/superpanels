//! Profile-body shape for `[[profile]]` blocks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::display::MonitorRef;
use crate::layout::{FitMode, ImageRectMm};
use crate::schedule::MonitorPlacement;

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
        /// Sparse per-image canvas snapshots — entries exist only for images
        /// the user hand-tuned. Keyed by the image's absolute path, so a
        /// rename or move drops the tweak (see `docs/followups.md`).
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        overrides: HashMap<PathBuf, ImageOverride>,
        /// Apply the profile-level layout (`SpanProfile::image_rect_mm` +
        /// `Profile::monitor_state`) to every image instead of cover-fitting
        /// each untuned image at its own aspect. Per-image `overrides` still
        /// win. Suits sets authored at one fixed resolution.
        #[serde(default, skip_serializing_if = "is_false")]
        uniform_layout: bool,
    },
}

#[allow(clippy::trivially_copy_pass_by_ref)] // reason: serde's skip_serializing_if takes &T
fn is_false(b: &bool) -> bool {
    !*b
}

impl SpanSource {
    /// The per-image canvas override for `image`, when one was authored.
    #[must_use]
    pub fn override_for(&self, image: &Path) -> Option<&ImageOverride> {
        match self {
            Self::Slideshow { overrides, .. } => overrides.get(image),
            Self::Single { .. } => None,
        }
    }
}

/// Per-image canvas snapshot inside a slideshow: the same placement + image
/// rect a profile persists at top level, applied only when this image is up.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct ImageOverride {
    /// Keys are `Monitor.stable_id` (or `name` fallback), matching
    /// [`super::Profile::monitor_state`].
    pub monitor_state: HashMap<String, MonitorPlacement>,
    pub image_rect_mm: ImageRectMm,
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
#[allow(clippy::panic)] // reason: same — explicit panic on unexpected enum branch
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

    fn slideshow_source_with_override(image: &str) -> SpanSource {
        let mut monitor_state = HashMap::new();
        monitor_state.insert(
            "uuid-a".to_owned(),
            MonitorPlacement {
                x_mm: 10.0,
                y_mm: 20.0,
            },
        );
        let mut overrides = HashMap::new();
        overrides.insert(
            PathBuf::from(image),
            ImageOverride {
                monitor_state,
                image_rect_mm: ImageRectMm {
                    x_mm: 1.0,
                    y_mm: 2.0,
                    w_mm: 1800.0,
                    h_mm: 600.0,
                },
            },
        );
        SpanSource::Slideshow {
            images: ImageSet::from_folder(PathBuf::from("/walls"), true),
            config: SlideshowConfig {
                interval: Duration::from_secs(600),
                sort: SlideshowSort::Shuffle,
                recent_history_size: 10,
                on_start: SlideshowStart::Resume,
                pause_when_active: false,
                skip_on_unavailable: true,
            },
            overrides,
            uniform_layout: false,
        }
    }

    #[test]
    fn slideshow_overrides_round_trip_through_toml() {
        // Profiles persist as TOML inside config.toml — that format, not
        // JSON, is the one that must hold path-keyed override tables.
        let source = slideshow_source_with_override("/walls/a.png");
        let toml = toml::to_string(&source).unwrap();
        let back: SpanSource = toml::from_str(&toml).unwrap();
        assert_eq!(back, source);
    }

    #[test]
    fn slideshow_overrides_round_trip_through_json() {
        let source = slideshow_source_with_override("/walls/a.png");
        let json = serde_json::to_string(&source).unwrap();
        let back: SpanSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, source);
    }

    #[test]
    fn slideshow_without_overrides_field_deserializes_to_empty_map() {
        // Pre-overrides configs and wire payloads must keep loading.
        let toml = concat!(
            "type = \"slideshow\"\n",
            "[images]\n",
            "sources = [{ type = \"folder\", path = \"/walls\" }]\n",
            "[config]\n",
            "interval_secs = 600\n",
        );
        let source: SpanSource = toml::from_str(toml).unwrap();
        let SpanSource::Slideshow { overrides, .. } = source else {
            panic!("expected slideshow source");
        };
        assert!(overrides.is_empty());
    }

    #[test]
    fn empty_overrides_are_not_serialized() {
        let source = SpanSource::Slideshow {
            images: ImageSet::from_folder(PathBuf::from("/walls"), false),
            config: SlideshowConfig {
                interval: Duration::from_secs(600),
                sort: SlideshowSort::Shuffle,
                recent_history_size: 10,
                on_start: SlideshowStart::Resume,
                pause_when_active: false,
                skip_on_unavailable: true,
            },
            overrides: HashMap::new(),
            uniform_layout: false,
        };
        let toml = toml::to_string(&source).unwrap();
        assert!(!toml.contains("overrides"), "got: {toml}");
        assert!(!toml.contains("uniform_layout"), "got: {toml}");
    }

    #[test]
    fn uniform_layout_round_trips_and_defaults_off() {
        let toml = concat!(
            "type = \"slideshow\"\n",
            "uniform_layout = true\n",
            "[images]\n",
            "sources = [{ type = \"folder\", path = \"/walls\" }]\n",
            "[config]\n",
            "interval_secs = 600\n",
        );
        let source: SpanSource = toml::from_str(toml).unwrap();
        let SpanSource::Slideshow { uniform_layout, .. } = &source else {
            panic!("expected slideshow source");
        };
        assert!(*uniform_layout);
        let back = toml::to_string(&source).unwrap();
        assert!(back.contains("uniform_layout = true"), "got: {back}");

        // Absent field defaults off (pre-uniform-layout configs keep loading).
        let source: SpanSource = toml::from_str(
            "type = \"slideshow\"\n[images]\nsources = []\n[config]\ninterval_secs = 600\n",
        )
        .unwrap();
        let SpanSource::Slideshow { uniform_layout, .. } = source else {
            panic!("expected slideshow source");
        };
        assert!(!uniform_layout);
    }

    #[test]
    fn override_for_matches_exact_path_only() {
        let source = slideshow_source_with_override("/walls/a.png");
        assert!(source.override_for(Path::new("/walls/a.png")).is_some());
        assert!(source.override_for(Path::new("/walls/b.png")).is_none());
    }

    #[test]
    fn override_for_on_single_source_is_none() {
        let source = SpanSource::Single {
            path: PathBuf::from("/walls/a.png"),
        };
        assert!(source.override_for(Path::new("/walls/a.png")).is_none());
    }
}
