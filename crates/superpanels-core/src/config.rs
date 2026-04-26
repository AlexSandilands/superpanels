//! On-disk configuration: profiles, per-monitor physical sizes, backend pin.
//!
//! Mirrors `SPEC.md` §14.1 (TOML schema) and §3.4 (profile shape). The Rust
//! types here are the source of truth for what the on-disk file may contain;
//! [`Config::load_or_default`] reads `$XDG_CONFIG_HOME/superpanels/config.toml`,
//! writing a documented default file if none exists yet.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::display::Monitor;
use crate::layout::BezelConfig;

mod default;
mod merge;
mod monitor_edit;
mod profile;
mod validate;

pub use monitor_edit::{MonitorEditError, MonitorIdentifier, diagonal_to_mm, write_monitor_block};
pub use profile::{ImageSet, ProfileBody, Schedule, SlideshowConfig, SpanSource};

const APP_DIR: &str = "superpanels";
const CONFIG_FILE: &str = "config.toml";

/// Top-level Superpanels configuration, loaded from
/// `$XDG_CONFIG_HOME/superpanels/config.toml`.
///
/// Mirrors `SPEC.md` §14.1. All sections have defaults so a missing or
/// minimal file is valid.
///
/// # Example
///
/// ```
/// # use superpanels_core::config::Config;
/// let cfg = Config::default();
/// assert_eq!(cfg.general.notifications, "errors");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// `[general]` table.
    #[serde(default)]
    pub general: GeneralConfig,
    /// `[backend]` table.
    #[serde(default)]
    pub backend: BackendConfig,
    /// `[library]` table.
    #[serde(default)]
    pub library: LibraryConfig,
    /// Repeated `[[monitor]]` blocks giving each known monitor its physical
    /// size in millimetres (`SPEC.md` §14.1).
    #[serde(default, rename = "monitor")]
    pub monitors: Vec<MonitorConfig>,
    /// Repeated `[[profile]]` blocks per `SPEC.md` §3.4.
    #[serde(default, rename = "profile")]
    pub profiles: Vec<Profile>,
}

/// `[general]` settings.
///
/// # Example
///
/// ```
/// # use superpanels_core::config::GeneralConfig;
/// let g = GeneralConfig::default();
/// assert_eq!(g.theme, "auto");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Profile to apply on daemon start; `None` means "do nothing".
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Whether to write `~/.config/autostart/superpanels.desktop` on first run.
    #[serde(default)]
    pub autostart: bool,
    /// `"off"`, `"errors"`, or `"all"`.
    #[serde(default = "default_notifications")]
    pub notifications: String,
    /// `"auto"`, `"light"`, or `"dark"`.
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

/// `[backend]` settings.
///
/// # Example
///
/// ```
/// # use superpanels_core::config::{BackendConfig, BackendKind};
/// let b = BackendConfig::default();
/// assert_eq!(b.prefer, BackendKind::Auto);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Pinned backend; `Auto` means run the detection ladder.
    #[serde(default)]
    pub prefer: BackendKind,
    /// Template for the custom backend (`{image_N}` / `{monitor_N}` placeholders).
    /// Only consulted when `prefer == Custom`.
    #[serde(default)]
    pub custom_command: String,
}

/// `[library]` settings.
///
/// # Example
///
/// ```
/// # use superpanels_core::config::LibraryConfig;
/// let l = LibraryConfig::default();
/// assert!(l.recursive);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryConfig {
    /// Folders the library scanner walks.
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    /// Recurse into subdirectories under each root.
    #[serde(default = "default_recursive")]
    pub recursive: bool,
    /// Edge length, in pixels, for cached thumbnails.
    #[serde(default = "default_thumbnail_size")]
    pub thumbnail_size: u32,
    /// Re-scan when the FS watcher reports changes under a root.
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
    320
}

fn default_auto_scan() -> bool {
    true
}

/// One `[[monitor]]` block: maps a runtime monitor to its physical size
/// in millimetres.
///
/// At least one of `stable_id` / `name` must be set; matching against a
/// detected [`Monitor`] tries `stable_id` first, then falls back to `name`
/// (`SPEC.md` §14.1).
///
/// # Example
///
/// ```
/// # use superpanels_core::config::MonitorConfig;
/// let m = MonitorConfig {
///     stable_id: None,
///     name: Some("DP-1".to_owned()),
///     physical_mm: [597, 336],
/// };
/// assert_eq!(m.physical_mm, [597, 336]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Compositor-supplied stable ID (KDE per-output UUID, or an EDID hash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    /// Output name (e.g. `"DP-1"`); used when `stable_id` is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Physical dimensions `[w, h]` in millimetres.
    pub physical_mm: [u32; 2],
}

/// Backend selection, mirrored to the on-disk `backend.prefer` string.
///
/// # Example
///
/// ```
/// # use superpanels_core::config::BackendKind;
/// let kind: BackendKind = "kde".parse().unwrap();
/// assert_eq!(kind, BackendKind::Kde);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Run the auto-detection ladder.
    #[default]
    Auto,
    /// KDE Plasma (zbus → `org.kde.PlasmaShell.evaluateScript`).
    Kde,
    /// GNOME Shell (`gsettings`-backed).
    Gnome,
    /// Sway / wlroots compositors (`swww` or `swaybg`).
    Sway,
    /// Hyprland (`hyprctl hyprpaper`).
    Hyprland,
    /// X11 fallback via `feh`.
    Feh,
    /// User-provided template command from [`BackendConfig::custom_command`].
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

/// One `[[profile]]` block per `SPEC.md` §3.4.
///
/// Profiles bundle the inputs needed to apply a wallpaper: the source
/// (single image vs slideshow vs per-monitor pinning), the bezel config,
/// an optional backend pin, and an optional schedule trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// User-facing name (must be unique across the config).
    pub name: String,
    /// Span vs per-monitor.
    pub body: ProfileBody,
    /// Per-profile bezel config (overrides nothing global; a sibling concept).
    pub bezels: BezelConfig,
    /// Optional backend pin; `None` means honour `[backend].prefer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_override: Option<BackendKind>,
    /// Optional time-of-day trigger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
}

/// Errors returned from config load / save / validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The on-disk file could not be read.
    #[error("could not read config at {path}: {source}")]
    Read {
        /// File the I/O attempt was against.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The on-disk file could not be written.
    #[error("could not write config at {path}: {source}")]
    Write {
        /// File the I/O attempt was against.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The TOML payload could not be deserialised.
    #[error("could not parse config at {path}: {message}")]
    Parse {
        /// File the parse attempt was against.
        path: PathBuf,
        /// Parser-supplied message (often line + column).
        message: String,
    },
    /// The TOML payload could not be serialised.
    #[error("could not serialise config: {0}")]
    Serialise(String),
    /// A field-level validation rule failed.
    #[error("invalid config at {path}: {field}: {message}")]
    Invalid {
        /// File the validation came from (or "<default>" / "<in-memory>").
        path: PathBuf,
        /// Dotted field path (e.g. `"profile[0].name"`, `"monitor[1]"`).
        field: String,
        /// Human-readable explanation.
        message: String,
    },
    /// `$XDG_CONFIG_HOME` and `$HOME` were both unset, so we couldn't pick
    /// a config-file path.
    #[error("could not determine config path: $XDG_CONFIG_HOME and $HOME both unset")]
    NoConfigDir,
}

impl Config {
    /// Load `Config` from the user's `$XDG_CONFIG_HOME/superpanels/config.toml`,
    /// writing a documented default file (with comments preserved by
    /// `toml_edit`) if none exists yet.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Read`] / [`ConfigError::Parse`] /
    /// [`ConfigError::Invalid`] / [`ConfigError::Write`] /
    /// [`ConfigError::NoConfigDir`] depending on which step fails.
    pub fn load_or_default() -> Result<Self, ConfigError> {
        let path = Self::default_path()?;
        Self::load_or_default_from(&path)
    }

    /// Test-friendly variant of [`Self::load_or_default`] that uses an
    /// explicit path instead of consulting the environment.
    ///
    /// # Errors
    ///
    /// As [`Self::load_or_default`].
    pub fn load_or_default_from(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            return Self::load_from(path);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::Write {
                path: parent.to_owned(),
                source: e,
            })?;
        }
        let default_text = default::default_document();
        fs::write(path, &default_text).map_err(|e| ConfigError::Write {
            path: path.to_owned(),
            source: e,
        })?;
        Self::load_from(path)
    }

    /// Read + parse + validate from an explicit path.
    ///
    /// # Errors
    ///
    /// As [`Self::load_or_default`], minus the directory-creation case.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|e| ConfigError::Read {
            path: path.to_owned(),
            source: e,
        })?;
        let cfg: Self = toml::from_str(&text).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            message: e.to_string(),
        })?;
        validate::validate(&cfg, path)?;
        Ok(cfg)
    }

    /// Serialise to TOML and write to the given path.
    ///
    /// Used by tests and by [`write_monitor_block`]'s post-write reload.
    /// Plain `toml::to_string` is used here — comment preservation is the
    /// job of [`write_monitor_block`], not the round-trip API.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Write`] / [`ConfigError::Serialise`].
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string(self).map_err(|e| ConfigError::Serialise(e.to_string()))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::Write {
                path: parent.to_owned(),
                source: e,
            })?;
        }
        fs::write(path, text).map_err(|e| ConfigError::Write {
            path: path.to_owned(),
            source: e,
        })
    }

    /// Default config path: `$XDG_CONFIG_HOME/superpanels/config.toml`,
    /// falling back to `$HOME/.config/superpanels/config.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::NoConfigDir`] when neither env var is set.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let p = PathBuf::from(xdg);
            if !p.as_os_str().is_empty() {
                return Ok(p.join(APP_DIR).join(CONFIG_FILE));
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            let p = PathBuf::from(home);
            if !p.as_os_str().is_empty() {
                return Ok(p.join(".config").join(APP_DIR).join(CONFIG_FILE));
            }
        }
        Err(ConfigError::NoConfigDir)
    }

    /// Walk the loaded `[[monitor]]` blocks and populate `physical_size_mm`
    /// (and derived `ppi`) on each detected [`Monitor`] that matches.
    ///
    /// Matching is by `stable_id` first, then `name` — the rule from
    /// `SPEC.md` §14.1.
    pub fn merge_into_monitors(&self, monitors: &mut [Monitor]) {
        merge::merge_monitor_config(&self.monitors, monitors);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on parse/io errors
#[allow(clippy::panic)] // reason: test asserts on a non-matching Result variant
mod tests {
    use super::*;
    use crate::display::{MonitorId, Rotation};
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
