//! Disk I/O for [`super::Config`].

use std::fs;
use std::path::{Path, PathBuf};

use super::{APP_DIR, CONFIG_FILE, Config, ConfigError, default, merge, validate};
use crate::display::Monitor;

impl Config {
    /// Load `Config`, writing a documented default file if none exists.
    pub fn load_or_default() -> Result<Self, ConfigError> {
        let path = Self::default_path()?;
        Self::load_or_default_from(&path)
    }

    /// Variant of [`Self::load_or_default`] using an explicit path.
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

    /// Serialise to TOML and write. Comments are not preserved — use
    /// [`super::write_monitor_block`] for round-trip with comments.
    ///
    /// Validates first: [`Self::load_from`] rejects an invalid file wholesale,
    /// so persisting one would brick every profile on the next start.
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        validate::validate(self, path)?;
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

    /// `$XDG_CONFIG_HOME/superpanels/config.toml`, falling back to
    /// `$HOME/.config/superpanels/config.toml`.
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

    /// Populate `physical_size_mm` / `ppi` from `[[monitor]]` blocks.
    /// Matches by `stable_id` first, then `name`.
    pub fn merge_into_monitors(&self, monitors: &mut [Monitor]) {
        merge::merge_monitor_config(&self.monitors, monitors);
    }
}
