//! Internal helpers for [`super::compute_crop_specs`].

use crate::display::{Monitor, MonitorRef, Rotation};
use crate::schedule::MonitorPlacement;

use super::{CropSliceSpec, FitMode, LayoutError, Rect};

pub(super) const MM_PER_INCH: f64 = 25.4;

const FLOAT_PIXEL_EPSILON: f64 = 1e-6;

fn is_valid_mm(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

pub(super) fn validate_inputs(
    monitors: &[Monitor],
    fit: FitMode,
    image_size: (u32, u32),
) -> Result<(), LayoutError> {
    if monitors.is_empty() {
        return Err(LayoutError::EmptyMonitorList);
    }

    let missing: Vec<MonitorRef> = monitors
        .iter()
        .filter(|m| m.physical_size_mm.is_none())
        .map(|m| MonitorRef {
            stable_id: m.stable_id.clone().unwrap_or_default(),
            name: m.name.clone(),
        })
        .collect();
    if !missing.is_empty() {
        return Err(LayoutError::PhysicalSizeMissing { monitors: missing });
    }

    for m in monitors {
        let (w_mm, h_mm) = m.physical_size_mm.unwrap_or((0.0, 0.0));
        if !is_valid_mm(w_mm) || !is_valid_mm(h_mm) {
            return Err(LayoutError::InvalidPhysicalSize {
                name: m.name.clone(),
            });
        }
    }

    match fit {
        FitMode::Fill | FitMode::Stretch => {}
        FitMode::Fit | FitMode::Center => {
            return Err(LayoutError::FitModeUnsupported { mode: fit });
        }
    }

    let (img_w, img_h) = image_size;
    if img_w == 0 || img_h == 0 {
        return Err(LayoutError::ImageTooSmall {
            image_w: img_w,
            image_h: img_h,
            canvas_w: 0,
            canvas_h: 0,
        });
    }

    Ok(())
}

/// Per-monitor data with rotation already applied.
pub(super) struct EffectiveMonitor {
    pub(super) width_mm: f64,
    pub(super) height_mm: f64,
    pub(super) pixel_w: u32,
    pub(super) pixel_h: u32,
    pub(super) placement: MonitorPlacement,
}

impl EffectiveMonitor {
    pub(super) fn from_monitor(m: &Monitor, placement: MonitorPlacement) -> Self {
        let phys_mm = m.physical_size_mm.unwrap_or((1.0, 1.0));
        let res_px = m.resolution;

        let (w_mm, h_mm, w_px, h_px) = match m.rotation {
            Rotation::None | Rotation::Inverted => (phys_mm.0, phys_mm.1, res_px.0, res_px.1),
            Rotation::Left | Rotation::Right => (phys_mm.1, phys_mm.0, res_px.1, res_px.0),
        };

        Self {
            width_mm: w_mm,
            height_mm: h_mm,
            pixel_w: w_px,
            pixel_h: h_px,
            placement,
        }
    }

    pub(super) fn ppi(&self) -> f64 {
        f64::from(self.pixel_w) / (self.width_mm / MM_PER_INCH)
    }
}

#[allow(clippy::struct_field_names)] // reason: per-axis fields share width/height suffixes by design
pub(super) struct CanvasPixels {
    pub(super) width_f: f64,
    pub(super) height_f: f64,
}

#[allow(clippy::similar_names)] // reason: width_mm / height_mm parallel arguments by design
pub(super) fn compute_canvas_pixels(
    canvas_width_mm: f64,
    canvas_height_mm: f64,
    reference_ppi: f64,
    image_size: (u32, u32),
) -> Result<CanvasPixels, LayoutError> {
    let width_f = canvas_width_mm * reference_ppi / MM_PER_INCH;
    let height_f = canvas_height_mm * reference_ppi / MM_PER_INCH;
    let (img_w, img_h) = image_size;
    let _ = mm_to_px_dim(width_f, img_w, img_h)?;
    let _ = mm_to_px_dim(height_f, img_w, img_h)?;
    Ok(CanvasPixels { width_f, height_f })
}

pub(super) struct SrcMapping {
    src_per_canvas: (f64, f64),
    src_origin: (f64, f64),
}

impl SrcMapping {
    pub(super) fn for_layout(
        fit: FitMode,
        canvas: &CanvasPixels,
        image_size: (u32, u32),
        image_size_px: Option<[u32; 2]>,
    ) -> Result<Self, LayoutError> {
        if let Some([w_px, h_px]) = image_size_px {
            let (img_w, img_h) = image_size;
            let sx = if w_px == 0 {
                f64::INFINITY
            } else {
                f64::from(img_w) / f64::from(w_px)
            };
            let sy = if h_px == 0 {
                f64::INFINITY
            } else {
                f64::from(img_h) / f64::from(h_px)
            };
            return Ok(Self {
                src_per_canvas: (sx, sy),
                src_origin: (0.0, 0.0),
            });
        }
        let (img_w, img_h) = image_size;
        let img_width = f64::from(img_w);
        let img_height = f64::from(img_h);
        match fit {
            FitMode::Stretch => Ok(Self {
                src_per_canvas: (img_width / canvas.width_f, img_height / canvas.height_f),
                src_origin: (0.0, 0.0),
            }),
            FitMode::Fill => {
                let s_w = canvas.width_f / img_width;
                let s_h = canvas.height_f / img_height;
                let s = s_w.max(s_h);
                let inv_s = 1.0 / s;
                let canvas_in_src_w = canvas.width_f * inv_s;
                let canvas_in_src_h = canvas.height_f * inv_s;
                let origin_x = (img_width - canvas_in_src_w) / 2.0;
                let origin_y = (img_height - canvas_in_src_h) / 2.0;
                Ok(Self {
                    src_per_canvas: (inv_s, inv_s),
                    src_origin: (origin_x, origin_y),
                })
            }
            FitMode::Fit | FitMode::Center => Err(LayoutError::FitModeUnsupported { mode: fit }),
        }
    }

    pub(super) fn monitor_to_slice(
        &self,
        origin_mm: (f64, f64),
        size_mm: (f64, f64),
        mon_dst_size: (u32, u32),
        reference_ppi: f64,
        image_size: (u32, u32),
        offset_px: [i32; 2],
    ) -> CropSliceSpec {
        let mm_to_canvas_px = reference_ppi / MM_PER_INCH;
        let canvas_x = origin_mm.0 * mm_to_canvas_px;
        let canvas_y = origin_mm.1 * mm_to_canvas_px;
        let canvas_w = size_mm.0 * mm_to_canvas_px;
        let canvas_h = size_mm.1 * mm_to_canvas_px;

        let unclamped = UnclampedSrc {
            left: self.src_origin.0 + (canvas_x - f64::from(offset_px[0])) * self.src_per_canvas.0,
            top: self.src_origin.1 + (canvas_y - f64::from(offset_px[1])) * self.src_per_canvas.1,
            width: canvas_w * self.src_per_canvas.0,
            height: canvas_h * self.src_per_canvas.1,
        };
        clamp_to_slice(&unclamped, image_size, mon_dst_size)
    }
}

struct UnclampedSrc {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

fn clamp_to_slice(
    src: &UnclampedSrc,
    image_size: (u32, u32),
    mon_dst_size: (u32, u32),
) -> CropSliceSpec {
    let img_extent_x = f64::from(image_size.0);
    let img_extent_y = f64::from(image_size.1);
    let (sx, dox, dsw) = clamp_axis(src.left, src.width, img_extent_x, mon_dst_size.0);
    let (sy, doy, dsh) = clamp_axis(src.top, src.height, img_extent_y, mon_dst_size.1);
    if dsw == 0 || dsh == 0 {
        return CropSliceSpec {
            src_rect: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            dst_offset: (0, 0),
            dst_size: (0, 0),
        };
    }
    let clamped_w = clamp_dim(sx, src.left, src.width, img_extent_x);
    let clamped_h = clamp_dim(sy, src.top, src.height, img_extent_y);
    CropSliceSpec {
        src_rect: Rect {
            x: round_to_u32(sx),
            y: round_to_u32(sy),
            w: round_to_u32(clamped_w),
            h: round_to_u32(clamped_h),
        },
        dst_offset: (dox, doy),
        dst_size: (dsw, dsh),
    }
}

fn clamp_axis(
    src_start: f64,
    src_extent: f64,
    img_extent: f64,
    mon_dst_extent: u32,
) -> (f64, u32, u32) {
    if !src_start.is_finite() || !src_extent.is_finite() || src_extent <= 0.0 {
        return (0.0, 0, 0);
    }
    let src_end = src_start + src_extent;
    if src_end <= 0.0 || src_start >= img_extent {
        return (0.0, 0, 0);
    }
    let src_per_dst = src_extent / f64::from(mon_dst_extent.max(1));
    let clipped_start = src_start.max(0.0);
    let clipped_end = src_end.min(img_extent);
    let dst_offset_f = ((clipped_start - src_start) / src_per_dst).max(0.0).round();
    let dst_size_f = ((clipped_end - clipped_start) / src_per_dst)
        .max(0.0)
        .round();
    let dst_offset = round_to_u32_clamped(dst_offset_f, mon_dst_extent);
    let dst_size = round_to_u32_clamped(dst_size_f, mon_dst_extent.saturating_sub(dst_offset));
    (clipped_start, dst_offset, dst_size)
}

fn clamp_dim(clamped_start: f64, src_start: f64, src_extent: f64, img_extent: f64) -> f64 {
    let src_end = src_start + src_extent;
    let clamped_end = src_end.min(img_extent);
    (clamped_end - clamped_start).max(0.0)
}

fn round_to_u32(v: f64) -> u32 {
    if !v.is_finite() || v < 0.0 {
        return 0;
    }
    let r = v.round();
    if r > f64::from(u32::MAX) {
        u32::MAX
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = r as u32;
        n
    }
}

fn round_to_u32_clamped(v: f64, max_value: u32) -> u32 {
    round_to_u32(v).min(max_value)
}

fn float_to_u32(
    v: f64,
    image_w: u32,
    image_h: u32,
    canvas_w: u32,
    canvas_h: u32,
) -> Result<u32, LayoutError> {
    if !v.is_finite() {
        return Err(LayoutError::ImageTooSmall {
            image_w,
            image_h,
            canvas_w,
            canvas_h,
        });
    }
    let v = if v < 0.0 && v > -FLOAT_PIXEL_EPSILON {
        0.0
    } else {
        v
    };
    if v < 0.0 {
        return Err(LayoutError::ImageTooSmall {
            image_w,
            image_h,
            canvas_w,
            canvas_h,
        });
    }
    let rounded = v.round_ties_even();
    if !(0.0..=f64::from(u32::MAX)).contains(&rounded) {
        return Err(LayoutError::ImageTooSmall {
            image_w,
            image_h,
            canvas_w,
            canvas_h,
        });
    }
    #[allow(clippy::cast_possible_truncation)] // reason: range checked above
    let as_i64 = rounded as i64;
    u32::try_from(as_i64).map_err(|_| LayoutError::ImageTooSmall {
        image_w,
        image_h,
        canvas_w,
        canvas_h,
    })
}

fn mm_to_px_dim(v: f64, image_w: u32, image_h: u32) -> Result<u32, LayoutError> {
    float_to_u32(v, image_w, image_h, 0, 0)
}
