//! Image load / scale / crop / rotate / `save_temp` pipeline.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageReader, imageops};
use thiserror::Error;

use crate::layout::{CropSpec, Rect};

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
    /// Pixel count exceeds the configured memory budget.
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

/// How [`scale_to_fit`] adapts a source image to the target dimensions.
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

/// Read the image header at `path` without decoding. Cheap (microseconds)
/// and the right primitive for any preview pipeline that only needs
/// dimensions.
pub fn read_dimensions(path: &Path) -> Result<(u32, u32), ImageError> {
    open_reader(path)?
        .into_dimensions()
        .map_err(|e| ImageError::Decode {
            path: path.to_owned(),
            message: e.to_string(),
        })
}

/// Resampling quality for [`load_thumbnail`].
#[derive(Debug, Clone, Copy)]
pub enum Resample {
    /// `Triangle`. Cheap enough to run once per library-grid tile.
    Fast,
    /// `Lanczos3`. Roughly 1.6× the cost of [`Resample::Fast`], and worth it
    /// wherever the result gets magnified again on screen — the GUI's preview
    /// canvas stretches one thumbnail across the whole desktop plane.
    High,
}

impl Resample {
    fn filter(self) -> imageops::FilterType {
        match self {
            Self::Fast => imageops::FilterType::Triangle,
            Self::High => imageops::FilterType::Lanczos3,
        }
    }
}

/// Decode `path` then downscale so the longest edge is at most `max_edge`
/// pixels. Used by the thumbnail IPC commands to keep payloads small. The
/// decode is gated by the default memory budget.
///
/// A source already within the box is returned at its native size —
/// `DynamicImage::resize` would otherwise *upscale* it to fill the box, paying
/// for pixels that carry no extra detail.
pub fn load_thumbnail(
    path: &Path,
    max_edge: u32,
    quality: Resample,
) -> Result<DynamicImage, ImageError> {
    let img = load(path)?;
    if img.width() <= max_edge && img.height() <= max_edge {
        return Ok(img);
    }
    Ok(img.resize(max_edge, max_edge, quality.filter()))
}

/// Encode `img` as PNG bytes for an IPC payload. In-memory counterpart to
/// [`save_temp`], but tuned differently: `Adaptive` row filtering costs about a
/// millisecond at thumbnail sizes yet shrinks a photographic 1536px encode ~6×,
/// which matters because these bytes cross IPC and are then held as a base64
/// `data:` URL in the webview.
pub fn encode_png(img: &DynamicImage) -> Result<Vec<u8>, ImageError> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut bytes = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::Adaptive);
    img.write_with_encoder(encoder)
        .map_err(|e| ImageError::Decode {
            path: PathBuf::new(),
            message: e.to_string(),
        })?;
    Ok(bytes)
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

/// Crop, scale, and letterbox a `source` image per `spec`. Skips
/// `compose_on_black` for the fully-covered legacy path so non-letterboxed
/// applies are byte-identical to the pre-Phase-4c pipeline.
/// Empty slices (the user
/// dragged the image entirely off-monitor) return an all-black canvas.
pub fn render_slice(source: &DynamicImage, spec: &CropSpec) -> Result<DynamicImage, ImageError> {
    if spec.slice_dst_size.0 == 0 || spec.slice_dst_size.1 == 0 {
        let (w, h) = spec.dst_size;
        return Ok(DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            w.max(1),
            h.max(1),
            image::Rgba([0, 0, 0, 255]),
        )));
    }
    let cropped = crop(source, spec.src_rect)?;
    let resized = scale_to_fit(&cropped, spec.slice_dst_size, FitMode::Stretch);
    if spec.needs_letterbox() {
        Ok(compose_on_black(&resized, spec.dst_size, spec.dst_offset))
    } else {
        Ok(resized)
    }
}

/// Compose `slice` onto a black `dst_size` canvas at `dst_offset`. Used by the
/// apply pipeline when the user has free-positioned the image and parts of a
/// monitor fall outside the source rect (
/// §4c.3). Pixels outside the slice stay opaque black.
#[must_use]
pub fn compose_on_black(
    slice: &DynamicImage,
    dst_size: (u32, u32),
    dst_offset: (u32, u32),
) -> DynamicImage {
    compose_on_fill(slice, dst_size, dst_offset, image::Rgba([0, 0, 0, 255]))
}

/// Compose `slice` onto a fully transparent `dst_size` canvas at `dst_offset`.
/// The covered sub-rect is opaque; everywhere else keeps zero alpha, so a stack
/// of these alpha-composites cleanly (lower layers show through the gaps). The
/// per-layer step of [`render_composite`].
#[must_use]
pub fn compose_on_transparent(
    slice: &DynamicImage,
    dst_size: (u32, u32),
    dst_offset: (u32, u32),
) -> DynamicImage {
    compose_on_fill(slice, dst_size, dst_offset, image::Rgba([0, 0, 0, 0]))
}

fn compose_on_fill(
    slice: &DynamicImage,
    dst_size: (u32, u32),
    dst_offset: (u32, u32),
    fill: image::Rgba<u8>,
) -> DynamicImage {
    let (dw, dh) = dst_size;
    let mut canvas = image::RgbaImage::from_pixel(dw.max(1), dh.max(1), fill);
    let (sw, sh) = (slice.width(), slice.height());
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return DynamicImage::ImageRgba8(canvas);
    }
    let max_w = dw.saturating_sub(dst_offset.0);
    let max_h = dh.saturating_sub(dst_offset.1);
    let paste_w = sw.min(max_w);
    let paste_h = sh.min(max_h);
    if paste_w == 0 || paste_h == 0 {
        return DynamicImage::ImageRgba8(canvas);
    }
    let cropped = slice.crop_imm(0, 0, paste_w, paste_h).to_rgba8();
    image::imageops::overlay(
        &mut canvas,
        &cropped,
        i64::from(dst_offset.0),
        i64::from(dst_offset.1),
    );
    DynamicImage::ImageRgba8(canvas)
}

/// One layer's contribution to a monitor, sized to the full destination
/// framebuffer with a *transparent* letterbox (unlike [`render_slice`], which
/// fills black). Stacking these with [`render_composite`] lets lower layers
/// show through wherever this one doesn't cover.
pub fn render_layer_slice(
    source: &DynamicImage,
    spec: &CropSpec,
) -> Result<DynamicImage, ImageError> {
    if spec.slice_dst_size.0 == 0 || spec.slice_dst_size.1 == 0 {
        let (w, h) = spec.dst_size;
        return Ok(DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            w.max(1),
            h.max(1),
            image::Rgba([0, 0, 0, 0]),
        )));
    }
    let cropped = crop(source, spec.src_rect)?;
    let resized = scale_to_fit(&cropped, spec.slice_dst_size, FitMode::Stretch);
    Ok(compose_on_transparent(
        &resized,
        spec.dst_size,
        spec.dst_offset,
    ))
}

/// Alpha-stack `layers` (bottom-to-top `(source, spec)` pairs) onto one
/// `dst_size` black monitor canvas. Each layer is rendered via
/// [`render_layer_slice`] and composited in order, so the topmost opaque pixels
/// win and uncovered regions stay black. Empty `layers` ⇒ an all-black canvas.
pub fn render_composite(
    layers: &[(&DynamicImage, &CropSpec)],
    dst_size: (u32, u32),
) -> Result<DynamicImage, ImageError> {
    let (dw, dh) = dst_size;
    let mut canvas =
        image::RgbaImage::from_pixel(dw.max(1), dh.max(1), image::Rgba([0, 0, 0, 255]));
    for (source, spec) in layers {
        let layer = render_layer_slice(source, spec)?;
        image::imageops::overlay(&mut canvas, &layer, 0, 0);
    }
    Ok(DynamicImage::ImageRgba8(canvas))
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
    use crate::display::Rotation;
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
    fn load_thumbnail_bounds_longest_edge_and_keeps_aspect() {
        // Arrange — 100x50 source, 40px cap on the long edge. A high-frequency
        // checkerboard, not a solid fill: the two filters agree pixel-for-pixel
        // on a flat image, so a solid fixture couldn't tell them apart.
        let dir = tempdir().unwrap();
        let mut src = RgbaImage::new(100, 50);
        for (x, y, p) in src.enumerate_pixels_mut() {
            let v = if (x + y) % 2 == 0 { 0 } else { 255 };
            *p = Rgba([v, v, v, 255]);
        }
        let path = write_png(dir.path(), "in.png", &DynamicImage::ImageRgba8(src));

        // Act
        let fast = load_thumbnail(&path, 40, Resample::Fast).unwrap();
        let high = load_thumbnail(&path, 40, Resample::High).unwrap();

        // Assert — both filters agree on geometry, and the quality knob is
        // really wired to the resample: Lanczos3 output differs from Triangle.
        assert_eq!((fast.width(), fast.height()), (40, 20));
        assert_eq!((high.width(), high.height()), (40, 20));
        assert_ne!(fast.to_rgba8().into_raw(), high.to_rgba8().into_raw());
    }

    #[test]
    fn load_thumbnail_does_not_upscale_a_smaller_source() {
        // Arrange — `resize` fits *within* the box, so a tiny source is
        // returned untouched rather than blown up to max_edge.
        let dir = tempdir().unwrap();
        let path = write_png(dir.path(), "in.png", &solid_image(8, 6, [0, 0, 0, 255]));

        // Act
        let out = load_thumbnail(&path, 1536, Resample::High).unwrap();

        // Assert
        assert_eq!((out.width(), out.height()), (8, 6));
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

    fn span_spec(
        src_rect: Rect,
        dst_offset: (u32, u32),
        dst_size: (u32, u32),
        slice_dst_size: (u32, u32),
    ) -> CropSpec {
        use crate::display::MonitorId;
        CropSpec {
            monitor_id: MonitorId(0),
            src_rect,
            dst_offset,
            dst_size,
            slice_dst_size,
            rotation: Rotation::None,
        }
    }

    #[test]
    fn render_slice_fully_covered_skips_compose() {
        // Arrange — a full-coverage spec; render_slice should produce a normal
        // resized image and *not* invoke compose_on_black (no black border).
        let src = solid_image(20, 10, [255, 0, 0, 255]);
        let spec = span_spec(
            Rect {
                x: 0,
                y: 0,
                w: 20,
                h: 10,
            },
            (0, 0),
            (40, 20),
            (40, 20),
        );

        // Act
        let out = render_slice(&src, &spec).unwrap();

        // Assert — every pixel is red; no black border around the slice.
        assert_eq!((out.width(), out.height()), (40, 20));
        let rgba = out.to_rgba8();
        for x in 0..40 {
            for y in 0..20 {
                assert_eq!(rgba.get_pixel(x, y).0, [255, 0, 0, 255]);
            }
        }
    }

    #[test]
    fn render_slice_letterbox_pads_with_black() {
        // Arrange — slice covers half the monitor, offset by 5,0.
        let src = solid_image(10, 10, [0, 0, 255, 255]);
        let spec = span_spec(
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            (5, 0),
            (20, 10),
            (10, 10),
        );

        // Act
        let out = render_slice(&src, &spec).unwrap();

        // Assert — full canvas is dst_size; left strip is black, right strip is blue.
        assert_eq!((out.width(), out.height()), (20, 10));
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(4, 0).0, [0, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(10, 0).0, [0, 0, 255, 255]);
    }

    #[test]
    fn render_slice_empty_slice_returns_full_black() {
        // Arrange — entirely off-image drag.
        let src = solid_image(10, 10, [255, 255, 0, 255]);
        let spec = span_spec(
            Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            (0, 0),
            (8, 6),
            (0, 0),
        );

        // Act
        let out = render_slice(&src, &spec).unwrap();

        // Assert
        assert_eq!((out.width(), out.height()), (8, 6));
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(7, 5).0, [0, 0, 0, 255]);
    }

    #[test]
    fn compose_on_black_pads_around_slice() {
        // Arrange — 4×4 red slice on a 10×10 canvas, offset (2, 3).
        let slice = solid_image(4, 4, [255, 0, 0, 255]);

        // Act
        let out = compose_on_black(&slice, (10, 10), (2, 3));

        // Assert — slice pixel lit red; outside pixel still black.
        assert_eq!((out.width(), out.height()), (10, 10));
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(3, 4).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(8, 8).0, [0, 0, 0, 255]);
    }

    #[test]
    fn compose_on_transparent_leaves_gaps_alpha_zero() {
        let slice = solid_image(4, 4, [255, 0, 0, 255]);
        let out = compose_on_transparent(&slice, (10, 10), (2, 3));
        let rgba = out.to_rgba8();
        // Covered pixel is opaque red; uncovered pixel is fully transparent.
        assert_eq!(rgba.get_pixel(3, 4).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(0, 0).0[3], 0);
    }

    #[test]
    fn render_composite_top_opaque_layer_hides_lower() {
        // Both layers fully cover a 10×10 monitor; the top (last) one wins.
        let red = solid_image(10, 10, [255, 0, 0, 255]);
        let blue = solid_image(10, 10, [0, 0, 255, 255]);
        let full = span_spec(
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            (0, 0),
            (10, 10),
            (10, 10),
        );
        let out = render_composite(&[(&red, &full), (&blue, &full)], (10, 10)).unwrap();
        assert_eq!(out.to_rgba8().get_pixel(5, 5).0, [0, 0, 255, 255]);
    }

    #[test]
    fn render_composite_partial_overlap_shows_both_layers() {
        // Layer 0 covers the left half (offset 0), layer 1 the right half
        // (offset 5). Neither overlaps the other, so both survive.
        let red = solid_image(5, 10, [255, 0, 0, 255]);
        let blue = solid_image(5, 10, [0, 0, 255, 255]);
        let left = span_spec(
            Rect {
                x: 0,
                y: 0,
                w: 5,
                h: 10,
            },
            (0, 0),
            (10, 10),
            (5, 10),
        );
        let right = span_spec(
            Rect {
                x: 0,
                y: 0,
                w: 5,
                h: 10,
            },
            (5, 0),
            (10, 10),
            (5, 10),
        );
        let out = render_composite(&[(&red, &left), (&blue, &right)], (10, 10)).unwrap();
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(1, 5).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(8, 5).0, [0, 0, 255, 255]);
    }

    #[test]
    fn render_composite_no_layers_is_all_black() {
        let out = render_composite(&[], (8, 6)).unwrap();
        assert_eq!((out.width(), out.height()), (8, 6));
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(7, 5).0, [0, 0, 0, 255]);
    }

    #[test]
    fn compose_on_black_clips_slice_extending_past_dst() {
        // Arrange — slice larger than the remaining dst region; the overflow
        // must clip rather than panic.
        let slice = solid_image(20, 20, [0, 255, 0, 255]);

        // Act
        let out = compose_on_black(&slice, (10, 10), (5, 5));

        // Assert — output stays at dst_size; the visible portion is the
        // top-left 5×5 of the slice composited at (5, 5).
        assert_eq!((out.width(), out.height()), (10, 10));
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(9, 9).0, [0, 255, 0, 255]);
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn compose_on_black_with_zero_slice_returns_full_black_canvas() {
        // Arrange — empty slice (0×0) means the monitor is fully off-image
        // and the temp file should be entirely black.
        let slice = solid_image(0, 0, [0, 0, 0, 0]);

        // Act
        let out = compose_on_black(&slice, (4, 4), (0, 0));

        // Assert
        assert_eq!((out.width(), out.height()), (4, 4));
        let rgba = out.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(3, 3).0, [0, 0, 0, 255]);
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
