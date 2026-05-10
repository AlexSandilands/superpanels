//! Internal helpers for [`super::compute_crop_specs`].

use crate::display::{Monitor, MonitorRef, Rotation};
use crate::schedule::MonitorPlacement;

use super::{CropSliceSpec, LayoutError, Rect};

fn is_valid_mm(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

pub(super) fn validate_inputs(
    monitors: &[Monitor],
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

    let (img_w, img_h) = image_size;
    if img_w == 0 || img_h == 0 {
        return Err(LayoutError::ImageZeroSize {
            image_w: img_w,
            image_h: img_h,
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
}

pub(super) struct UnclampedSrc {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

pub(super) fn clamp_to_slice(
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
