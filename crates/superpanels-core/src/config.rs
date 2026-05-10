//! On-disk config (`docs/spec/14-config-state.md`).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::schedule::{MonitorPlacement, ProfileColour, Schedule, TopologyFingerprint};

mod default;
mod io;
mod merge;
mod monitor_edit;
mod profile;
mod validate;

pub use monitor_edit::{MonitorEditError, MonitorIdentifier, diagonal_to_mm, write_monitor_block};
pub use profile::{
    ImageSet, PerMonitorAssignment, PerMonitorProfile, ProfileBody, ProfileTimestamp,
    SlideshowConfig, SlideshowSort, SlideshowStart, SpanProfile, SpanSource, now_timestamp,
};

const APP_DIR: &str = "superpanels";
const CONFIG_FILE: &str = "config.toml";

/// Loaded from `$XDG_CONFIG_HOME/superpanels/config.toml`. All sections have
/// defaults, so a missing or minimal file is valid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default, rename = "monitor")]
    pub monitors: Vec<MonitorConfig>,
    #[serde(default, rename = "profile")]
    pub profiles: Vec<Profile>,
    #[serde(default, rename = "schedule")]
    pub schedules: Vec<Schedule>,
    /// Master pause for all schedules; mirrored in the tray menu.
    #[serde(default)]
    pub schedules_paused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct GeneralConfig {
    /// Profile to apply on daemon start when `state.toml` has no
    /// `active_profile` recorded; `None` means do nothing.
    #[serde(default)]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_notifications")]
    pub notifications: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_profile: None,
            autostart: false,
            notifications: default_notifications(),
            theme: default_theme(),
        }
    }
}

fn default_notifications() -> String {
    "errors".to_owned()
}

fn default_theme() -> String {
    "auto".to_owned()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct BackendConfig {
    #[serde(default)]
    pub prefer: BackendKind,
    #[serde(default)]
    pub custom_command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct LibraryConfig {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    #[serde(default = "default_recursive")]
    pub recursive: bool,
    #[serde(default = "default_thumbnail_size")]
    pub thumbnail_size: u32,
    #[serde(default = "default_auto_scan")]
    pub auto_scan: bool,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            recursive: default_recursive(),
            thumbnail_size: default_thumbnail_size(),
            auto_scan: default_auto_scan(),
        }
    }
}

fn default_recursive() -> bool {
    true
}

fn default_thumbnail_size() -> u32 {
    512
}

fn default_auto_scan() -> bool {
    true
}

/// One `[[monitor]]` block. At least one of `stable_id` / `name` must be set;
/// matching tries `stable_id` first, then falls back to `name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct MonitorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `[w, h]` in millimetres. Fractional values accepted.
    #[ts(type = "[number, number]")]
    pub physical_mm: [f64; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    #[default]
    Auto,
    Kde,
    Gnome,
    Sway,
    Hyprland,
    Feh,
    Custom,
}

impl std::str::FromStr for BackendKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "kde" => Ok(Self::Kde),
            "gnome" => Ok(Self::Gnome),
            "sway" => Ok(Self::Sway),
            "hyprland" => Ok(Self::Hyprland),
            "feh" => Ok(Self::Feh),
            "custom" => Ok(Self::Custom),
            other => Err(format!("unknown backend `{other}`")),
        }
    }
}

/// One `[[profile]]` block. Profiles are *modes the user is in*, not
/// one-shot apply requests: monitor placements, image transform, and colour
/// swatch all live here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct Profile {
    pub name: String,
    pub body: ProfileBody,
    /// Per-monitor canvas state: physical mm placements + per-monitor
    /// rotation. Keys are `Monitor.stable_id` (or `name` fallback).
    #[serde(default)]
    pub monitor_state: HashMap<String, MonitorPlacement>,
    /// Topology fingerprint at authoring time; compared for equality at
    /// apply time. Persisted as opaque hash hex.
    pub topology: TopologyFingerprint,
    #[serde(default)]
    pub colour: ProfileColour,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "now_timestamp")]
    pub created_at: ProfileTimestamp,
    #[serde(default = "now_timestamp")]
    pub updated_at: ProfileTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_at: Option<ProfileTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_override: Option<BackendKind>,
}

impl Profile {
    /// Touch `updated_at` to `now`. Call from any mutating IPC method.
    pub fn touch(&mut self) {
        self.updated_at = now_timestamp();
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read config at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write config at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse config at {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("could not serialise config: {0}")]
    Serialise(String),
    #[error("invalid config at {path}: {field}: {message}")]
    Invalid {
        path: PathBuf,
        field: String,
        message: String,
    },
    #[error("could not determine config path: $XDG_CONFIG_HOME and $HOME both unset")]
    NoConfigDir,
}
