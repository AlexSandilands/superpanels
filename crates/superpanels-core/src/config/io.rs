//! Disk I/O for [`super::Config`]: load-or-default, save, default path,
//! and the runtime monitor merge.
//!
//! Carved out of `config.rs` to keep that file under the 600-line cap.
//! All public methods stay attached to `Config` (this file only adds an
//! `impl` block); call sites do not change.

use std::fs;
use std::path::{Path, PathBuf};

use super::{APP_DIR, CONFIG_FILE, Config, ConfigError, default, merge, validate};
use crate::display::Monitor;

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
    /// Used by tests and by [`super::write_monitor_block`]'s post-write reload.
    /// Plain `toml::to_string` is used here — comment preservation is the
    /// job of [`super::write_monitor_block`], not the round-trip API.
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
