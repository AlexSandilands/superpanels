//! GNOME Shell backend (`SPEC.md` §10.4). Composites per-monitor crops into
//! one image and points `picture-uri[-dark]` at it via `gsettings`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{DynamicImage, GenericImageView, RgbaImage, imageops};
use tracing::{debug, info};

use crate::display::{Availability, MonitorRef};

use super::subprocess::{DEFAULT_TIMEOUT, run, which};
use super::{AppliedReport, BackendError, WallpaperBackend};

const NAME: &str = "gnome";
const TOOL: &str = "gsettings";
const SCHEMA: &str = "org.gnome.desktop.background";
const KEY_LIGHT: &str = "picture-uri";
const KEY_DARK: &str = "picture-uri-dark";
/// Composite long-edge cap to keep memory bounded.
pub(crate) const MAX_LONG_EDGE: u32 = 8192;

#[derive(Debug, Default)]
pub struct GnomeBackend;

impl GnomeBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for GnomeBackend {
    // reason: trait method signature is `&str`; the constant is incidental.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        NAME
    }

    fn availability(&self) -> Availability {
        let xdg_ok = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .is_some_and(|d| d.split(':').any(|s| s.eq_ignore_ascii_case("GNOME")));
        if !xdg_ok {
            return Availability::WrongEnvironment {
                reason: "$XDG_CURRENT_DESKTOP does not contain GNOME",
            };
        }
        if !which(TOOL) {
            return Availability::ToolMissing { tool: TOOL };
        }
        Availability::Available
    }

    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError> {
        if assignments.is_empty() {
            return Ok(AppliedReport {
                monitors_set: 0,
                duration: std::time::Duration::ZERO,
                backend: NAME,
            });
        }
        let avail = self.availability();
        if avail != Availability::Available {
            return Err(BackendError::Unavailable {
                backend: NAME,
                reason: format!("availability check returned {avail:?}"),
            });
        }
        let started = Instant::now();
        let composite_path = composite_to_tempfile(assignments)?;
        let uri = path_to_file_uri(&composite_path);
        debug!(uri = %uri, backend = NAME, "setting picture-uri");
        gsettings_set(KEY_LIGHT, &uri)?;
        gsettings_set(KEY_DARK, &uri)?;
        let duration = started.elapsed();
        info!(monitors = assignments.len(), backend = NAME, "applied");
        Ok(AppliedReport {
            monitors_set: assignments.len(),
            duration,
            backend: NAME,
        })
    }

    fn supports_per_monitor(&self) -> bool {
        false
    }
}

fn gsettings_set(key: &str, value: &str) -> Result<(), BackendError> {
    // gsettings parses its value as GVariant; strings need single quotes.
    // `Command::arg` is shell-free so we wrap explicitly. Safe here because
    // `value` is a `file://` URI we built under our own cache dir.
    let value_arg = format!("'{value}'");
    let args: [&OsStr; 4] = [
        OsStr::new("set"),
        OsStr::new(SCHEMA),
        OsStr::new(key),
        OsStr::new(value_arg.as_str()),
    ];
    run(TOOL, &args, DEFAULT_TIMEOUT).map(|_| ())
}

pub(crate) fn composite_to_tempfile(
    assignments: &[(MonitorRef, PathBuf)],
) -> Result<PathBuf, BackendError> {
    let mut decoded: Vec<DynamicImage> = Vec::with_capacity(assignments.len());
    for (_, path) in assignments {
        let img = image::open(path).map_err(|e| {
            BackendError::Encode(format!("could not decode crop `{}`: {e}", path.display()))
        })?;
        decoded.push(img);
    }
    let canvas = composite_band(&decoded)?;
    let scaled = downscale_if_needed(canvas, MAX_LONG_EDGE);
    let out_path = pick_output_path()?;
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            BackendError::Encode(format!(
                "could not create temp dir `{}`: {e}",
                parent.display()
            ))
        })?;
    }
    scaled.save(&out_path).map_err(|e| {
        BackendError::Encode(format!(
            "could not write composite `{}`: {e}",
            out_path.display()
        ))
    })?;
    Ok(out_path)
}

/// Width = sum of per-crop widths, height = max per-crop height; each crop
/// is pasted top-aligned at the running horizontal offset.
pub(crate) fn composite_band(decoded: &[DynamicImage]) -> Result<RgbaImage, BackendError> {
    if decoded.is_empty() {
        return Err(BackendError::Encode(
            "cannot composite empty crop list".to_owned(),
        ));
    }
    let mut total_w: u32 = 0;
    let mut max_h: u32 = 0;
    for img in decoded {
        let (w, h) = img.dimensions();
        total_w = total_w
            .checked_add(w)
            .ok_or_else(|| BackendError::Encode("composite width overflow".to_owned()))?;
        if h > max_h {
            max_h = h;
        }
    }
    if total_w == 0 || max_h == 0 {
        return Err(BackendError::Encode(
            "composite would have a zero edge".to_owned(),
        ));
    }
    let mut canvas: RgbaImage = RgbaImage::new(total_w, max_h);
    let mut x_off: i64 = 0;
    for img in decoded {
        let rgba = img.to_rgba8();
        imageops::overlay(&mut canvas, &rgba, x_off, 0);
        let w_i64 = i64::from(rgba.width());
        x_off = x_off
            .checked_add(w_i64)
            .ok_or_else(|| BackendError::Encode("composite x-offset overflow".to_owned()))?;
    }
    Ok(canvas)
}

pub(crate) fn downscale_if_needed(canvas: RgbaImage, cap: u32) -> DynamicImage {
    let (w, h) = (canvas.width(), canvas.height());
    let long = w.max(h);
    if long <= cap || cap == 0 {
        return DynamicImage::ImageRgba8(canvas);
    }
    let new_w = scale_dim(w, cap, long);
    let new_h = scale_dim(h, cap, long);
    DynamicImage::ImageRgba8(canvas).resize_exact(new_w, new_h, imageops::FilterType::Lanczos3)
}

/// `dim * cap / long`, clamped into `1..=cap` so extreme aspect ratios don't
/// collapse a dimension to zero. u64 intermediate avoids overflow.
fn scale_dim(dim: u32, cap: u32, long: u32) -> u32 {
    let scaled = u64::from(dim).saturating_mul(u64::from(cap)) / u64::from(long.max(1));
    let scaled_u32 = u32::try_from(scaled).unwrap_or(cap);
    scaled_u32.clamp(1, cap)
}

fn pick_output_path() -> Result<PathBuf, BackendError> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .ok_or_else(|| {
            BackendError::Encode(
                "could not determine cache dir: $XDG_CACHE_HOME and $HOME both unset".to_owned(),
            )
        })?;
    Ok(base
        .join("superpanels")
        .join("temp")
        .join("gnome-composite.png"))
}

fn path_to_file_uri(path: &Path) -> String {
    // No percent-encoding: gsettings accepts unencoded paths and we control the path.
    format!("file://{}", path.display())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on encode/io errors
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(width: u32, height: u32, red: u8, green: u8, blue: u8) -> DynamicImage {
        let mut img = RgbaImage::new(width, height);
        for px in img.pixels_mut() {
            *px = Rgba([red, green, blue, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn name_is_stable_and_per_monitor_is_false() {
        let b = GnomeBackend::new();
        assert_eq!(b.name(), "gnome");
        assert!(!b.supports_per_monitor());
    }

    #[test]
    fn composite_band_lays_out_left_to_right_with_max_height() {
        let imgs = vec![solid(10, 5, 255, 0, 0), solid(7, 8, 0, 255, 0)];
        let canvas = composite_band(&imgs).unwrap();
        assert_eq!(canvas.width(), 17);
        assert_eq!(canvas.height(), 8);
        assert_eq!(canvas.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(canvas.get_pixel(10, 0), &Rgba([0, 255, 0, 255]));
    }

    #[test]
    fn composite_band_rejects_empty_input() {
        let err = composite_band(&[]).unwrap_err();
        assert!(matches!(err, BackendError::Encode(_)));
    }

    #[test]
    fn downscale_no_op_when_under_cap() {
        let canvas = RgbaImage::new(100, 50);
        let out = downscale_if_needed(canvas, 8192);
        assert_eq!(out.dimensions(), (100, 50));
    }

    #[test]
    fn downscale_scales_long_edge_to_cap() {
        // Use small dimensions (same 4:1 ratio) — verifies the math, not allocation size.
        let canvas = RgbaImage::new(128, 32);
        let out = downscale_if_needed(canvas, 64);
        assert_eq!(out.width(), 64);
        assert_eq!(out.height(), 16);
    }

    #[test]
    fn empty_apply_returns_zero() {
        let report = GnomeBackend::new().apply(&[]).unwrap();
        assert_eq!(report.monitors_set, 0);
    }

    #[test]
    fn path_to_file_uri_prefixes_correctly() {
        let s = path_to_file_uri(Path::new("/walls/x.png"));
        assert_eq!(s, "file:///walls/x.png");
    }
}
