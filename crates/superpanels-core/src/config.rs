//! On-disk config (`SPEC.md` §14.1, §3.4).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::layout::BezelConfig;

mod default;
mod io;
mod merge;
mod monitor_edit;
mod profile;
mod validate;

pub use monitor_edit::{MonitorEditError, MonitorIdentifier, diagonal_to_mm, write_monitor_block};
pub use profile::{
    ImageSet, PerMonitorAssignment, PerMonitorProfile, ProfileBody, Schedule, SlideshowConfig,
    SlideshowSort, SlideshowStart, SpanProfile, SpanSource,
};

const APP_DIR: &str = "superpanels";
const CONFIG_FILE: &str = "config.toml";

/// Loaded from `$XDG_CONFIG_HOME/superpanels/config.toml`. All sections have
/// defaults, so a missing or minimal file is valid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Profile to apply on daemon start; `None` means do nothing.
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Write `~/.config/autostart/superpanels.desktop` on first run.
    #[serde(default)]
    pub autostart: bool,
    /// `"off"` | `"errors"` | `"all"`.
    #[serde(default = "default_notifications")]
    pub notifications: String,
    /// `"auto"` | `"light"` | `"dark"`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// `[latitude, longitude]` in decimal degrees. Required for `Sunset` schedules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat_lon: Option<[f64; 2]>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_profile: None,
            autostart: false,
            notifications: default_notifications(),
            theme: default_theme(),
            lat_lon: None,
        }
    }
}

fn default_notifications() -> String {
    "errors".to_owned()
}

fn default_theme() -> String {
    "auto".to_owned()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(default)]
    pub prefer: BackendKind,
    /// Template with `{image_N}` / `{monitor_N}` placeholders. Only used when
    /// `prefer == Custom`.
    #[serde(default)]
    pub custom_command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryConfig {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    #[serde(default = "default_recursive")]
    pub recursive: bool,
    /// Cached thumbnail edge in pixels.
    #[serde(default = "default_thumbnail_size")]
    pub thumbnail_size: u32,
    /// Re-scan on FS watcher events.
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
    // 512 px gives a crisp tile at 2x DPR with up to ~256 px CSS-pixel tiles
    // in the library grid; the encode cost is still well under a frame and
    // the on-disk thumb cache is bounded so size growth is tolerable.
    512
}

fn default_auto_scan() -> bool {
    true
}

/// One `[[monitor]]` block. At least one of `stable_id` / `name` must be set;
/// matching tries `stable_id` first, then falls back to `name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `[w, h]` in millimetres.
    pub physical_mm: [u32; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    #[default]
    Auto,
    Kde,
    Gnome,
    Sway,
    Hyprland,
    Feh,
    /// User-provided template from [`BackendConfig::custom_command`].
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

/// One `[[profile]]` block (`SPEC.md` §3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Must be unique across the config.
    pub name: String,
    pub body: ProfileBody,
    pub bezels: BezelConfig,
    /// `None` honours `[backend].prefer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_override: Option<BackendKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
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
    /// `field` is a dotted path like `profile[0].name`.
    #[error("invalid config at {path}: {field}: {message}")]
    Invalid {
        path: PathBuf,
        field: String,
        message: String,
    },
    #[error("could not determine config path: $XDG_CONFIG_HOME and $HOME both unset")]
    NoConfigDir,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on parse/io errors
#[allow(clippy::panic)] // reason: test asserts on a non-matching Result variant
mod tests {
    use super::*;
    use crate::display::{Monitor, MonitorId, Rotation};
    use crate::layout::BezelConfig;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_profile(name: &str) -> Profile {
        Profile {
            name: name.to_owned(),
            body: ProfileBody::Span(profile::SpanProfile {
                source: SpanSource::Single {
                    path: PathBuf::from("/tmp/x.jpg"),
                },
                fit: crate::layout::FitMode::Fill,
                offset: [0, 0],
            }),
            bezels: BezelConfig {
                horizontal_mm: 8.0,
                vertical_mm: 5.0,
            },
            backend_override: None,
            schedule: None,
        }
    }

    fn sample_config() -> Config {
        Config {
            general: GeneralConfig {
                default_profile: Some("home".to_owned()),
                autostart: false,
                notifications: "errors".to_owned(),
                theme: "auto".to_owned(),
                lat_lon: None,
            },
            backend: BackendConfig {
                prefer: BackendKind::Auto,
                custom_command: String::new(),
            },
            library: LibraryConfig {
                roots: vec![PathBuf::from("/home/u/walls")],
                recursive: true,
                thumbnail_size: 320,
                auto_scan: true,
            },
            monitors: vec![MonitorConfig {
                stable_id: Some("uuid-1".to_owned()),
                name: Some("DP-1".to_owned()),
                physical_mm: [597, 336],
            }],
            profiles: vec![sample_profile("home")],
        }
    }

    #[test]
    fn round_trip_through_toml_preserves_value() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = sample_config();

        // Act
        original.save_to(&path).unwrap();
        let mut roundtrip = Config::load_from(&path).unwrap();
        roundtrip.profiles[0].name = "renamed".to_owned();
        roundtrip.save_to(&path).unwrap();
        let final_load = Config::load_from(&path).unwrap();

        // Assert
        assert_eq!(final_load.profiles[0].name, "renamed");
        assert_eq!(final_load.monitors[0].physical_mm, [597, 336]);
        assert_eq!(final_load.general.theme, "auto");
    }

    #[test]
    fn missing_file_writes_default_then_loads_successfully() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        assert!(!path.exists());

        // Act
        let cfg = Config::load_or_default_from(&path).unwrap();

        // Assert
        assert!(path.exists());
        assert_eq!(cfg.general.notifications, "errors");
    }

    #[test]
    fn monitor_without_id_or_name_returns_invalid_error() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let bad = "[[monitor]]\nphysical_mm = [597, 336]\n";
        std::fs::write(&path, bad).unwrap();

        // Act
        let result = Config::load_from(&path);

        // Assert
        assert!(
            matches!(result, Err(ConfigError::Invalid { ref field, .. }) if field == "monitor[0]")
        );
    }

    #[test]
    fn duplicate_profile_names_return_invalid_error() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = sample_config();
        cfg.profiles.push(sample_profile("home"));
        cfg.save_to(&path).unwrap();

        // Act
        let result = Config::load_from(&path);

        // Assert
        let Err(ConfigError::Invalid { field, .. }) = result else {
            panic!("expected Invalid, got {result:?}");
        };
        assert!(field.starts_with("profile["), "field was {field}");
    }

    #[test]
    fn negative_bezel_returns_invalid_error() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = sample_config();
        cfg.profiles[0].bezels.horizontal_mm = -1.0;
        cfg.save_to(&path).unwrap();

        // Act
        let result = Config::load_from(&path);

        // Assert
        let Err(ConfigError::Invalid { field, .. }) = result else {
            panic!("expected Invalid, got {result:?}");
        };
        assert!(field.contains("bezels"), "field was {field}");
    }

    #[test]
    fn merge_populates_physical_size_when_stable_id_matches() {
        // Arrange
        let cfg = sample_config();
        let mut monitors = vec![Monitor {
            id: MonitorId(0),
            name: "DP-9".to_owned(),
            stable_id: Some("uuid-1".to_owned()),
            position: (0, 0),
            resolution: (2560, 1440),
            physical_size_mm: None,
            scale: 1.0,
            rotation: Rotation::None,
            refresh_hz: None,
            primary: true,
            ppi: None,
        }];

        // Act
        cfg.merge_into_monitors(&mut monitors);

        // Assert
        assert_eq!(monitors[0].physical_size_mm, Some((597, 336)));
        assert!(monitors[0].ppi.is_some());
    }

    #[test]
    fn merge_falls_back_to_name_when_stable_id_absent() {
        // Arrange
        let cfg = sample_config();
        let mut monitors = vec![Monitor {
            id: MonitorId(0),
            name: "DP-1".to_owned(),
            stable_id: None,
            position: (0, 0),
            resolution: (1920, 1080),
            physical_size_mm: None,
            scale: 1.0,
            rotation: Rotation::None,
            refresh_hz: None,
            primary: false,
            ppi: None,
        }];

        // Act
        cfg.merge_into_monitors(&mut monitors);

        // Assert
        assert_eq!(monitors[0].physical_size_mm, Some((597, 336)));
    }
}
