//! Profile-body shape for `[[profile]]` blocks (`docs/spec/03-core-concepts.md`
//! §3.4).

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::display::MonitorRef;
use crate::layout::FitMode;

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
    #[serde(default)]
    pub fit: FitMode,
    /// Image-position offset in canvas px.
    #[serde(default)]
    pub offset: [i32; 2],
    /// Explicit image rectangle in canvas px (`docs/spec/12-gui.md` §12.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_size_px: Option<[u32; 2]>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
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
