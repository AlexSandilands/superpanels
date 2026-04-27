//! Image load / scale / crop / rotate / `save_temp` pipeline (`SPEC.md` §8).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageReader, imageops};
use thiserror::Error;

use crate::display::Rotation;
use crate::layout::Rect;

mod temp;

pub use temp::{clear_temp_dir, clear_temp_dir_at, default_temp_dir, save_temp, save_temp_in};

const DEFAULT_DECODE_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
const BYTES_PER_PIXEL: u64 = 4;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("io on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not decode {path}: {message}")]
    Decode { path: PathBuf, message: String },
    /// Pixel count exceeds the configured memory budget (`SPEC.md` §8.6).
    #[error(
        "image {path} would need {needed_bytes} bytes ({width}x{height}) — over the {budget_bytes}-byte cap"
    )]
    TooBig {
        path: PathBuf,
        width: u32,
        height: u32,
        needed_bytes: u64,
        budget_bytes: u64,
    },
    #[error(
        "crop rect {rect_x},{rect_y} {rect_w}x{rect_h} runs outside image bounds {image_w}x{image_h}"
    )]
    CropOutOfBounds {
        rect_x: u32,
        rect_y: u32,
        rect_w: u32,
        rect_h: u32,
        image_w: u32,
        image_h: u32,
    },
    #[error("could not determine cache dir: $XDG_CACHE_HOME and $HOME both unset")]
    NoCacheDir,
}

/// How [`scale_to_fit`] adapts a source image to the target dimensions (`SPEC.md` §8.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FitMode {
    #[default]
    Fill,
    Fit,
    Stretch,
    Center,
}

/// Decode the image at `path`, refusing files whose decoded pixel count
/// exceeds the default memory budget (512 MiB at 4 bpp).
pub fn load(path: &Path) -> Result<DynamicImage, ImageError> {
    load_with_budget(path, DEFAULT_DECODE_BUDGET_BYTES)
}

/// Variant of [`load`] with an explicit budget — used by tests to exercise
/// the [`ImageError::TooBig`] path without crafting a multi-gigapixel file.
pub fn load_with_budget(path: &Path, budget_bytes: u64) -> Result<DynamicImage, ImageError> {
    let dims_reader = open_reader(path)?;
    let (w, h) = dims_reader
        .into_dimensions()
        .map_err(|e| ImageError::Decode {
            path: path.to_owned(),
            message: e.to_string(),
        })?;
    let needed = u64::from(w) * u64::from(h) * BYTES_PER_PIXEL;
    if needed > budget_bytes {
        return Err(ImageError::TooBig {
            path: path.to_owned(),
            width: w,
            height: h,
            needed_bytes: needed,
            budget_bytes,
        });
    }
    let reader = open_reader(path)?;
    reader.decode().map_err(|e| ImageError::Decode {
        path: path.to_owned(),
        message: e.to_string(),
    })
}

fn open_reader(path: &Path) -> Result<ImageReader<std::io::BufReader<fs::File>>, ImageError> {
    ImageReader::open(path)
        .map_err(|e| ImageError::Io {
            path: path.to_owned(),
            source: e,
        })?
        .with_guessed_format()
        .map_err(|e| ImageError::Io {
            path: path.to_owned(),
            source: e,
        })
}

/// Scale `img` to `target` according to `mode`. Lanczos3 sampling.
#[must_use]
pub fn scale_to_fit(img: &DynamicImage, target: (u32, u32), mode: FitMode) -> DynamicImage {
    let (tw, th) = target;
    if tw == 0 || th == 0 {
        return img.clone();
    }
    match mode {
        FitMode::Stretch => img.resize_exact(tw, th, imageops::FilterType::Lanczos3),
        FitMode::Fill => {
            let scaled = scale_to_cover(img, (tw, th));
            crop_centre(&scaled, (tw, th))
        }
        FitMode::Fit => {
            let scaled = img.resize(tw, th, imageops::FilterType::Lanczos3);
            paste_onto_black(&scaled, (tw, th))
        }
        FitMode::Center => paste_onto_black(img, (tw, th)),
    }
}

fn scale_to_cover(img: &DynamicImage, target: (u32, u32)) -> DynamicImage {
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 {
        return img.clone();
    }
    let (tw, th) = target;
    let sx = f64::from(tw) / f64::from(iw);
    let sy = f64::from(th) / f64::from(ih);
    let scale = sx.max(sy);
    let new_w = u32_from_f64((f64::from(iw) * scale).ceil()).unwrap_or(tw);
    let new_h = u32_from_f64((f64::from(ih) * scale).ceil()).unwrap_or(th);
    img.resize_exact(new_w.max(tw), new_h.max(th), imageops::FilterType::Lanczos3)
}

fn crop_centre(img: &DynamicImage, target: (u32, u32)) -> DynamicImage {
    let (iw, ih) = (img.width(), img.height());
    let (tw, th) = (target.0.min(iw), target.1.min(ih));
    let x = (iw - tw) / 2;
    let y = (ih - th) / 2;
    img.crop_imm(x, y, tw, th)
}

fn paste_onto_black(img: &DynamicImage, target: (u32, u32)) -> DynamicImage {
    let (tw, th) = target;
    let mut canvas = image::RgbaImage::from_pixel(tw, th, image::Rgba([0, 0, 0, 255]));
    let (iw, ih) = (img.width().min(tw), img.height().min(th));
    let x = (tw - iw) / 2;
    let y = (th - ih) / 2;
    let cropped = img.crop_imm(0, 0, iw, ih).to_rgba8();
    image::imageops::overlay(&mut canvas, &cropped, i64::from(x), i64::from(y));
    DynamicImage::ImageRgba8(canvas)
}

/// Crop `img` to `rect`. Zero-sized rects are rejected.
pub fn crop(img: &DynamicImage, rect: Rect) -> Result<DynamicImage, ImageError> {
    let (iw, ih) = (img.width(), img.height());
    let x_end = rect.x.saturating_add(rect.w);
    let y_end = rect.y.saturating_add(rect.h);
    if rect.w == 0 || rect.h == 0 || x_end > iw || y_end > ih {
        return Err(ImageError::CropOutOfBounds {
            rect_x: rect.x,
            rect_y: rect.y,
            rect_w: rect.w,
            rect_h: rect.h,
            image_w: iw,
            image_h: ih,
        });
    }
    Ok(img.crop_imm(rect.x, rect.y, rect.w, rect.h))
}

/// Rotate `img` by 90/180/270 °. `Rotation::None` clones.
#[must_use]
pub fn rotate(img: &DynamicImage, rotation: Rotation) -> DynamicImage {
    match rotation {
        Rotation::None => img.clone(),
        Rotation::Right => DynamicImage::ImageRgba8(imageops::rotate90(img)),
        Rotation::Inverted => DynamicImage::ImageRgba8(imageops::rotate180(img)),
        Rotation::Left => DynamicImage::ImageRgba8(imageops::rotate270(img)),
    }
}

fn u32_from_f64(v: f64) -> Option<u32> {
    if v.is_finite() && v >= 0.0 && v <= f64::from(u32::MAX) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // reason: range checked
        let n = v as u32;
        Some(n)
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on io errors
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use tempfile::tempdir;

    fn solid_image(w: u32, h: u32, colour: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(colour)))
    }

    fn write_png(dir: &Path, name: &str, img: &DynamicImage) -> PathBuf {
        let path = dir.join(name);
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn load_round_trips_a_small_png() {
        // Arrange
        let dir = tempdir().unwrap();
        let original = solid_image(8, 6, [10, 20, 30, 255]);
        let path = write_png(dir.path(), "in.png", &original);

        // Act
        let loaded = load(&path).unwrap();

        // Assert
        assert_eq!((loaded.width(), loaded.height()), (8, 6));
    }

    #[test]
    fn load_with_tight_budget_returns_too_big_error() {
        // Arrange — a 100x60 image needs 24,000 bytes; budget is 1.
        let dir = tempdir().unwrap();
        let path = write_png(dir.path(), "in.png", &solid_image(100, 60, [0, 0, 0, 255]));

        // Act
        let result = load_with_budget(&path, 1);

        // Assert
        assert!(matches!(result, Err(ImageError::TooBig { .. })));
    }

    #[test]
    fn load_missing_file_returns_io_error() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("nope.png");

        // Act
        let result = load(&path);

        // Assert
        assert!(matches!(result, Err(ImageError::Io { .. })));
    }

    #[test]
    fn scale_to_fit_fill_preserves_target_dimensions() {
        // Arrange
        let src = solid_image(100, 50, [255, 0, 0, 255]);

        // Act
        let out = scale_to_fit(&src, (200, 200), FitMode::Fill);

        // Assert
        assert_eq!((out.width(), out.height()), (200, 200));
    }

    #[test]
    fn scale_to_fit_stretch_distorts_to_target() {
        // Arrange
        let src = solid_image(10, 10, [0, 255, 0, 255]);

        // Act
        let out = scale_to_fit(&src, (40, 5), FitMode::Stretch);

        // Assert
        assert_eq!((out.width(), out.height()), (40, 5));
    }

    #[test]
    fn scale_to_fit_fit_letterboxes_into_target() {
        // Arrange — wide source, square target → letterbox bars top/bottom.
        let src = solid_image(40, 10, [255, 255, 0, 255]);

        // Act
        let out = scale_to_fit(&src, (40, 40), FitMode::Fit);

        // Assert
        assert_eq!((out.width(), out.height()), (40, 40));
    }

    #[test]
    fn scale_to_fit_center_does_not_resample() {
        // Arrange
        let src = solid_image(10, 10, [128, 128, 128, 255]);

        // Act
        let out = scale_to_fit(&src, (40, 40), FitMode::Center);

        // Assert
        assert_eq!((out.width(), out.height()), (40, 40));
    }

    #[test]
    fn crop_within_bounds_returns_subimage() {
        // Arrange
        let src = solid_image(10, 10, [0, 0, 255, 255]);

        // Act
        let out = crop(
            &src,
            Rect {
                x: 1,
                y: 2,
                w: 4,
                h: 5,
            },
        )
        .unwrap();

        // Assert
        assert_eq!((out.width(), out.height()), (4, 5));
    }

    #[test]
    fn crop_outside_bounds_returns_error() {
        // Arrange
        let src = solid_image(4, 4, [0, 0, 0, 255]);

        // Act
        let result = crop(
            &src,
            Rect {
                x: 0,
                y: 0,
                w: 5,
                h: 5,
            },
        );

        // Assert
        assert!(matches!(result, Err(ImageError::CropOutOfBounds { .. })));
    }

    #[test]
    fn crop_zero_size_returns_error() {
        // Arrange
        let src = solid_image(4, 4, [0, 0, 0, 255]);

        // Act
        let result = crop(
            &src,
            Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 4,
            },
        );

        // Assert
        assert!(matches!(result, Err(ImageError::CropOutOfBounds { .. })));
    }

    #[test]
    fn rotate_right_swaps_dimensions() {
        // Arrange
        let src = solid_image(2, 4, [0, 0, 0, 255]);

        // Act
        let out = rotate(&src, Rotation::Right);

        // Assert
        assert_eq!((out.width(), out.height()), (4, 2));
    }

    #[test]
    fn rotate_none_returns_same_dimensions() {
        // Arrange
        let src = solid_image(2, 4, [0, 0, 0, 255]);

        // Act
        let out = rotate(&src, Rotation::None);

        // Assert
        assert_eq!((out.width(), out.height()), (2, 4));
    }

    #[test]
    fn save_temp_in_writes_a_readable_png() {
        // Arrange
        let dir = tempdir().unwrap();
        let img = solid_image(5, 5, [255, 0, 255, 255]);

        // Act
        let path = save_temp_in(&img, "out.png", dir.path()).unwrap();

        // Assert
        assert!(path.exists());
        let reread = load(&path).unwrap();
        assert_eq!((reread.width(), reread.height()), (5, 5));
    }

    #[test]
    fn clear_temp_dir_at_removes_existing_files() {
        // Arrange
        let dir = tempdir().unwrap();
        let temp = dir.path().join("temp");
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join("stale.png"), b"junk").unwrap();
        assert!(temp.join("stale.png").exists());

        // Act
        clear_temp_dir_at(&temp).unwrap();

        // Assert
        assert!(temp.exists());
        assert!(!temp.join("stale.png").exists());
    }
}
