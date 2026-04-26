//! Temp-dir helpers for the image pipeline: `save_temp`, `clear_temp_dir`,
//! `default_temp_dir`, and their explicit-path test variants.
//!
//! Carved out of `image.rs` to keep that file under the 600-line cap. The
//! `*_at` / `*_in` variants exist so tests can drive the pipeline against
//! `tempfile::tempdir()` without touching `$XDG_CACHE_HOME`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::DynamicImage;

use super::ImageError;

const APP_DIR: &str = "superpanels";
const TEMP_DIR: &str = "temp";

/// Write `img` as PNG into the user's superpanels temp dir under `name`.
///
/// Returns the full path written. The caller has previously cleared the
/// directory via [`clear_temp_dir`] (`SPEC.md` §8.5).
///
/// # Errors
///
/// Returns [`ImageError::NoCacheDir`] when `$XDG_CACHE_HOME` and `$HOME`
/// are both unset; [`ImageError::Io`] on a write failure.
pub fn save_temp(img: &DynamicImage, name: &str) -> Result<PathBuf, ImageError> {
    let dir = default_temp_dir()?;
    save_temp_in(img, name, &dir)
}

/// Variant of [`save_temp`] that writes into an explicit directory — used
/// by tests so they don't touch `$XDG_CACHE_HOME`.
///
/// # Errors
///
/// As [`save_temp`], minus the `$XDG_CACHE_HOME` lookup.
pub fn save_temp_in(img: &DynamicImage, name: &str, dir: &Path) -> Result<PathBuf, ImageError> {
    fs::create_dir_all(dir).map_err(|e| ImageError::Io {
        path: dir.to_owned(),
        source: e,
    })?;
    let path = dir.join(name);
    img.save(&path).map_err(|e| ImageError::Io {
        path: path.clone(),
        source: io::Error::other(e.to_string()),
    })?;
    Ok(path)
}

/// Atomically clear (remove + recreate) the user's superpanels temp dir.
///
/// Called at the start of an apply so the previous run's files can't be
/// picked up by accident (`SPEC.md` §8.5).
///
/// # Errors
///
/// Returns [`ImageError::NoCacheDir`] / [`ImageError::Io`] depending on
/// which step fails.
pub fn clear_temp_dir() -> Result<(), ImageError> {
    let dir = default_temp_dir()?;
    clear_temp_dir_at(&dir)
}

/// Variant of [`clear_temp_dir`] for tests that pass an explicit path.
///
/// # Errors
///
/// As [`clear_temp_dir`].
pub fn clear_temp_dir_at(dir: &Path) -> Result<(), ImageError> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| ImageError::Io {
            path: dir.to_owned(),
            source: e,
        })?;
    }
    fs::create_dir_all(dir).map_err(|e| ImageError::Io {
        path: dir.to_owned(),
        source: e,
    })
}

/// Default temp dir: `$XDG_CACHE_HOME/superpanels/temp/`, falling back to
/// `$HOME/.cache/superpanels/temp/`.
///
/// # Errors
///
/// Returns [`ImageError::NoCacheDir`] when neither env var is set.
pub fn default_temp_dir() -> Result<PathBuf, ImageError> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        let p = PathBuf::from(xdg);
        if !p.as_os_str().is_empty() {
            return Ok(p.join(APP_DIR).join(TEMP_DIR));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home);
        if !p.as_os_str().is_empty() {
            return Ok(p.join(".cache").join(APP_DIR).join(TEMP_DIR));
        }
    }
    Err(ImageError::NoCacheDir)
}
