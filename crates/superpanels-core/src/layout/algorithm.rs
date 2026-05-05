//! Internal helpers for [`super::compute_crop_specs`].

use crate::display::{Monitor, MonitorRef, Rotation};

use super::{BezelConfig, CropSliceSpec, FitMode, LayoutError, Rect};

pub(super) const MM_PER_INCH: f64 = 25.4;

/// One millionth of a pixel — used to collapse self-cancelling products to zero.
const FLOAT_PIXEL_EPSILON: f64 = 1e-6;

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
        // Safe: presence checked above.
        let (w_mm, h_mm) = m.physical_size_mm.unwrap_or((0, 0));
        if w_mm == 0 || h_mm == 0 {
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
    /// Logical-pixel y-extent on the desktop, used for row grouping.
    pos_y: i64,
    pos_y_end: i64,
    pos_x: i64,
}

impl EffectiveMonitor {
    pub(super) fn from_monitor(m: &Monitor) -> Self {
        // Presence validated by `validate_inputs`; (1, 1) keeps us out of div-by-zero.
        let phys_mm = m.physical_size_mm.unwrap_or((1, 1));
        let res_px = m.resolution;

        let (w_mm, h_mm, w_px, h_px) = match m.rotation {
            Rotation::None | Rotation::Inverted => (phys_mm.0, phys_mm.1, res_px.0, res_px.1),
            Rotation::Left | Rotation::Right => (phys_mm.1, phys_mm.0, res_px.1, res_px.0),
        };

        let pos_x = i64::from(m.position.0);
        let pos_y = i64::from(m.position.1);
        let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
        let logical_h_f = (f64::from(h_px) / scale).round_ties_even();
        let logical_h =
            if logical_h_f.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&logical_h_f) {
                #[allow(clippy::cast_possible_truncation)] // reason: range checked above
                let value = logical_h_f as i64;
                value
            } else {
                i64::from(h_px)
            };
        let pos_y_end = pos_y.saturating_add(logical_h.max(1));

        Self {
            width_mm: f64::from(w_mm),
            height_mm: f64::from(h_mm),
            pixel_w: w_px,
            pixel_h: h_px,
            pos_y,
            pos_y_end,
            pos_x,
        }
    }

    pub(super) fn ppi(&self) -> f64 {
        // Width-axis density. Per-axis rounding noise would inflate canvas dims.
        f64::from(self.pixel_w) / (self.width_mm / MM_PER_INCH)
    }
}

/// Group monitors into rows by y-overlap. Rows top-to-bottom, monitors left-to-right.
pub(super) fn group_into_rows(effs: &[EffectiveMonitor]) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..effs.len()).collect();
    order.sort_by_key(|&i| effs[i].pos_y);

    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut row_extents: Vec<(i64, i64)> = Vec::new();
    'outer: for i in order {
        let (a, b) = (effs[i].pos_y, effs[i].pos_y_end);
        for (row_idx, ext) in row_extents.iter_mut().enumerate() {
            // Half-open interval overlap.
            if a < ext.1 && ext.0 < b {
                rows[row_idx].push(i);
                ext.0 = ext.0.min(a);
                ext.1 = ext.1.max(b);
                continue 'outer;
            }
        }
        rows.push(vec![i]);
        row_extents.push((a, b));
    }

    let mut row_order: Vec<usize> = (0..rows.len()).collect();
    row_order.sort_by_key(|&r| row_extents[r].0);

    let mut sorted_rows: Vec<Vec<usize>> = row_order.into_iter().map(|r| rows[r].clone()).collect();

    for row in &mut sorted_rows {
        row.sort_by_key(|&i| effs[i].pos_x);
    }

    sorted_rows
}

pub(super) fn build_row_index(rows: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut idx = vec![0usize; n];
    for (r, row) in rows.iter().enumerate() {
        for &i in row {
            idx[i] = r;
        }
    }
    idx
}

/// Mm-space layout (all fields in millimetres).
#[allow(clippy::struct_field_names)] // reason: `_mm` makes units unambiguous in arithmetic
pub(super) struct CanvasLayout {
    pub(super) x_origin_mm: Vec<f64>,
    pub(super) row_y_mm: Vec<f64>,
    pub(super) canvas_w_mm: f64,
    pub(super) canvas_h_mm: f64,
}

pub(super) fn compute_canvas_layout(
    effs: &[EffectiveMonitor],
    rows: &[Vec<usize>],
    bezels: BezelConfig,
) -> CanvasLayout {
    let mut x_origin_mm = vec![0.0_f64; effs.len()];
    let mut row_widths_mm = Vec::with_capacity(rows.len());
    let mut row_heights_mm = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cursor = 0.0_f64;
        let mut max_h = 0.0_f64;
        for (within_idx, &i) in row.iter().enumerate() {
            x_origin_mm[i] = cursor;
            cursor += effs[i].width_mm;
            if within_idx + 1 < row.len() {
                cursor += f64::from(bezels.horizontal_mm);
            }
            if effs[i].height_mm > max_h {
                max_h = effs[i].height_mm;
            }
        }
        row_widths_mm.push(cursor);
        row_heights_mm.push(max_h);
    }

    let mut row_y_mm = Vec::with_capacity(rows.len());
    let mut cursor_y = 0.0_f64;
    for (i, &h) in row_heights_mm.iter().enumerate() {
        row_y_mm.push(cursor_y);
        cursor_y += h;
        if i + 1 < rows.len() {
            cursor_y += f64::from(bezels.vertical_mm);
        }
    }

    let canvas_w_mm = row_widths_mm.iter().copied().fold(0.0_f64, f64::max);
    CanvasLayout {
        x_origin_mm,
        row_y_mm,
        canvas_w_mm,
        canvas_h_mm: cursor_y,
    }
}

/// Reference-PPI pixel dimensions of the canvas, as floats for downstream
/// src-mapping. The `mm_to_px_dim` validation in `compute_canvas_pixels`
/// surfaces `ImageTooSmall` when the canvas-px dimension overflows `u32`.
#[allow(clippy::struct_field_names)] // reason: per-axis fields share width/height suffixes by design
pub(super) struct CanvasPixels {
    pub(super) width_f: f64,
    pub(super) height_f: f64,
}

pub(super) fn compute_canvas_pixels(
    layout: &CanvasLayout,
    reference_ppi: f64,
    image_size: (u32, u32),
) -> Result<CanvasPixels, LayoutError> {
    let width_f = layout.canvas_w_mm * reference_ppi / MM_PER_INCH;
    let height_f = layout.canvas_h_mm * reference_ppi / MM_PER_INCH;
    let (img_w, img_h) = image_size;
    let _ = mm_to_px_dim(width_f, img_w, img_h)?;
    let _ = mm_to_px_dim(height_f, img_w, img_h)?;
    Ok(CanvasPixels { width_f, height_f })
}

/// Canvas-pixel → source-pixel mapping. Per-axis so Stretch (axis-independent)
/// and Fill (uniform cover, centred) are both expressible.
pub(super) struct SrcMapping {
    src_per_canvas: (f64, f64),
    src_origin: (f64, f64),
}

impl SrcMapping {
    /// Build the canvas-px → source-px mapping for either the FitMode-driven
    /// legacy path (`image_size_px = None`) or the GUI's free-transform path
    /// (`image_size_px = Some([w, h])`, see Phase 4c §4c.2).
    pub(super) fn for_layout(
        fit: FitMode,
        canvas: &CanvasPixels,
        image_size: (u32, u32),
        image_size_px: Option<[u32; 2]>,
    ) -> Result<Self, LayoutError> {
        if let Some([w_px, h_px]) = image_size_px {
            // Free transform: the image rect is `(offset, image_size_px)` in
            // canvas px and maps to the full source image. Zero dimensions mean
            // nothing covers anywhere; treat them as a degenerate-but-valid
            // mapping with infinite src-per-canvas so clamping yields empty
            // src_rects on every monitor.
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

    /// Map a single monitor onto the source image, clamping the resulting
    /// rectangle to the image bounds. Out-of-image regions become non-zero
    /// `dst_offset`/reduced `dst_size` so the apply layer can letterbox.
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

/// Pre-clamp source rectangle in float pixels, before bounding to the image.
struct UnclampedSrc {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

/// Clamp the un-clamped source rectangle to the image, deriving the matching
/// destination region. Anything outside the image becomes letterbox padding —
/// `dst_offset` for clipping at the start, reduced `dst_size` at the end.
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

/// Clamp a single axis. Returns `(clamped_src_start, dst_offset, dst_size)`.
/// `src_per_dst` is positive; tiny epsilon-noise around 0 is collapsed.
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
        // reason: range checked above
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = r as u32;
        n
    }
}

fn round_to_u32_clamped(v: f64, max_value: u32) -> u32 {
    round_to_u32(v).min(max_value)
}

/// Tiny negatives within `FLOAT_PIXEL_EPSILON` clamp to zero — they come from
/// self-cancelling products like `(image_w - image_w * scale * inv_scale) / 2`.
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

/// `float_to_u32` for canvas dimensions; the report prints `0` for canvas-px.
fn mm_to_px_dim(v: f64, image_w: u32, image_h: u32) -> Result<u32, LayoutError> {
    float_to_u32(v, image_w, image_h, 0, 0)
}
