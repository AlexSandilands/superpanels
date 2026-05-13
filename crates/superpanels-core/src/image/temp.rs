//! Temp-dir helpers for the image pipeline.

use std::fs;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use image::DynamicImage;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};

use super::ImageError;

const APP_DIR: &str = "superpanels";
const TEMP_DIR: &str = "temp";

/// Write `img` as PNG into the superpanels temp dir under `name`.
pub fn save_temp(img: &DynamicImage, name: &str) -> Result<PathBuf, ImageError> {
    let dir = default_temp_dir()?;
    save_temp_in(img, name, &dir)
}

/// Variant of [`save_temp`] that writes into an explicit directory.
///
/// Uses a fast PNG encoder (`CompressionType::Fast`, `FilterType::NoFilter`)
/// because the slice is consumed once by the wallpaper backend and then
/// wiped on the next apply — paying for default deflate compression to
/// shrink an ephemeral file costs seconds per monitor at wallpaper sizes.
pub fn save_temp_in(img: &DynamicImage, name: &str, dir: &Path) -> Result<PathBuf, ImageError> {
    fs::create_dir_all(dir).map_err(|e| ImageError::Io {
        path: dir.to_owned(),
        source: e,
    })?;
    let path = dir.join(name);
    let file = fs::File::create(&path).map_err(|e| ImageError::Io {
        path: path.clone(),
        source: e,
    })?;
    let writer = BufWriter::new(file);
    let encoder = PngEncoder::new_with_quality(writer, CompressionType::Fast, FilterType::NoFilter);
    img.write_with_encoder(encoder)
        .map_err(|e| ImageError::Io {
            path: path.clone(),
            source: io::Error::other(e.to_string()),
        })?;
    Ok(path)
}

/// Remove and recreate the superpanels temp dir; called at the start of each apply.
pub fn clear_temp_dir() -> Result<(), ImageError> {
    let dir = default_temp_dir()?;
    clear_temp_dir_at(&dir)
}

/// Variant of [`clear_temp_dir`] that operates on an explicit path.
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

/// `$XDG_CACHE_HOME/superpanels/temp/`, falling back to `$HOME/.cache/superpanels/temp/`.
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
