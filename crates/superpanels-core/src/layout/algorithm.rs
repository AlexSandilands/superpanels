//! Internal helpers for [`super::compute_crop_specs`].
//!
//! The split is structural — the public API and data types live in
//! `layout.rs`, the mm/px arithmetic and helpers live here — so each file
//! stays under the 600-line hard limit (`docs/architecture.md`).

use crate::display::{Monitor, MonitorRef, Rotation};

use super::{BezelConfig, FitMode, LayoutError, Rect};

pub(super) const MM_PER_INCH: f64 = 25.4;

/// Tolerance for sub-pixel floating-point error when collapsing values that
/// should be exactly zero (e.g. an image origin computed from a self-cancelling
/// product). Sized at one millionth of a pixel — well below any meaningful
/// rounding effect on a u32 dimension.
const FLOAT_PIXEL_EPSILON: f64 = 1e-6;

/// Pre-flight checks: monitor list, missing/zero physical sizes, supported
/// fit mode, non-zero image dims.
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
    /// Width in mm with rotation applied (post-rotation).
    pub(super) width_mm: f64,
    /// Height in mm with rotation applied.
    pub(super) height_mm: f64,
    /// Pixel width post-rotation.
    pub(super) pixel_w: u32,
    /// Pixel height post-rotation.
    pub(super) pixel_h: u32,
    /// Logical-pixel y-extent on the desktop, used for row grouping.
    pos_y: i64,
    /// Logical-pixel y-extent (top-exclusive).
    pos_y_end: i64,
    /// Logical-pixel x-origin, used for left-to-right ordering within a row.
    pos_x: i64,
}

impl EffectiveMonitor {
    pub(super) fn from_monitor(m: &Monitor) -> Self {
        // Presence validated by `validate_inputs`; default to (1, 1)
        // defensively so accidental misuse never divides by zero.
        let phys_mm = m.physical_size_mm.unwrap_or((1, 1));
        let res_px = m.resolution;

        let (w_mm, h_mm, w_px, h_px) = match m.rotation {
            Rotation::None | Rotation::Inverted => (phys_mm.0, phys_mm.1, res_px.0, res_px.1),
            Rotation::Left | Rotation::Right => (phys_mm.1, phys_mm.0, res_px.1, res_px.0),
        };

        let pos_x = i64::from(m.position.0);
        let pos_y = i64::from(m.position.1);
        // Logical pixels = post-rotation pixels divided by scale, rounded.
        // Tiny mismatches don't affect overlap detection.
        let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
        let logical_h_f = (f64::from(h_px) / scale).round_ties_even();
        // f64::from(u32::MAX) is exactly representable; the bounded range
        // makes the `as i64` cast lossless.
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
        // One PPI per monitor: width-axis density. Pixel aspect and physical
        // aspect agree by design on real panels (modulo mm being recorded as
        // integers); using a single axis keeps canvas dimensions consistent
        // with each monitor's own pixel count instead of inflating them by
        // per-axis rounding noise.
        f64::from(self.pixel_w) / (self.width_mm / MM_PER_INCH)
    }
}

/// Group monitors into rows by y-overlap of their logical-pixel extents.
///
/// Returns a `Vec<row>` where each row is a `Vec<usize>` of indices into
/// `effs`, sorted top-to-bottom (rows) then left-to-right (within row).
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

/// Build a reverse index: monitor index → row index.
pub(super) fn build_row_index(rows: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut idx = vec![0usize; n];
    for (r, row) in rows.iter().enumerate() {
        for &i in row {
            idx[i] = r;
        }
    }
    idx
}

/// Mm-space layout: per-monitor x-origin and per-row y-origin/height,
/// plus the canvas extents. All fields are in millimetres.
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

/// Reference-PPI pixel dimensions of the canvas, both as floats (for the
/// downstream src-mapping arithmetic) and as the rounded `u32` values that
/// appear in error reports.
#[allow(clippy::struct_field_names)] // reason: per-axis fields share width/height suffixes by design
pub(super) struct CanvasPixels {
    pub(super) width_f: f64,
    pub(super) height_f: f64,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn compute_canvas_pixels(
    layout: &CanvasLayout,
    reference_ppi: f64,
    image_size: (u32, u32),
) -> Result<CanvasPixels, LayoutError> {
    let width_f = layout.canvas_w_mm * reference_ppi / MM_PER_INCH;
    let height_f = layout.canvas_h_mm * reference_ppi / MM_PER_INCH;
    let (img_w, img_h) = image_size;
    let width = mm_to_px_dim(width_f, img_w, img_h)?;
    let height = mm_to_px_dim(height_f, img_w, img_h)?;
    Ok(CanvasPixels {
        width_f,
        height_f,
        width,
        height,
    })
}

/// Linear mapping from canvas-pixel coordinates to source-image-pixel
/// coordinates. Each axis has its own scale and origin so Stretch (axis-
/// independent) and Fill (uniform cover, centered) are both expressible.
pub(super) struct SrcMapping {
    src_per_canvas: (f64, f64),
    src_origin: (f64, f64),
}

impl SrcMapping {
    pub(super) fn for_fit(
        fit: FitMode,
        canvas: &CanvasPixels,
        image_size: (u32, u32),
    ) -> Result<Self, LayoutError> {
        let (img_w, img_h) = image_size;
        let img_width = f64::from(img_w);
        let img_height = f64::from(img_h);
        match fit {
            FitMode::Stretch => Ok(Self {
                src_per_canvas: (img_width / canvas.width_f, img_height / canvas.height_f),
                src_origin: (0.0, 0.0),
            }),
            FitMode::Fill => {
                // Cover ratio: source image scaled by `s` covers the canvas.
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

    pub(super) fn monitor_to_src_rect(
        &self,
        origin_mm: (f64, f64),
        size_mm: (f64, f64),
        reference_ppi: f64,
        image_size: (u32, u32),
        canvas_pixels: (u32, u32),
    ) -> Result<Rect, LayoutError> {
        let (img_w, img_h) = image_size;
        let (cv_w, cv_h) = canvas_pixels;
        let mm_to_canvas_px = reference_ppi / MM_PER_INCH;
        let canvas_x = origin_mm.0 * mm_to_canvas_px;
        let canvas_y = origin_mm.1 * mm_to_canvas_px;
        let canvas_w = size_mm.0 * mm_to_canvas_px;
        let canvas_h = size_mm.1 * mm_to_canvas_px;

        let src_left = self.src_origin.0 + canvas_x * self.src_per_canvas.0;
        let src_top = self.src_origin.1 + canvas_y * self.src_per_canvas.1;
        let src_width = canvas_w * self.src_per_canvas.0;
        let src_height = canvas_h * self.src_per_canvas.1;

        Ok(Rect {
            x: float_to_u32(src_left, img_w, img_h, cv_w, cv_h)?,
            y: float_to_u32(src_top, img_w, img_h, cv_w, cv_h)?,
            w: float_to_u32(src_width, img_w, img_h, cv_w, cv_h)?,
            h: float_to_u32(src_height, img_w, img_h, cv_w, cv_h)?,
        })
    }
}

/// Convert a non-negative float pixel dimension to `u32`, returning
/// [`LayoutError::ImageTooSmall`] if it is non-finite or out of range.
///
/// Tiny negative values (within `FLOAT_PIXEL_EPSILON`) are clamped to zero
/// — they originate from arithmetic that should mathematically yield zero
/// (e.g. `(image_w - image_w * scale * inv_scale) / 2`) but accumulates a
/// sub-pixel rounding error.
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
    // Bound to a range where the `as i64` cast is exact (any f64 outside
    // [0.0, u32::MAX] is rejected as `ImageTooSmall`).
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

/// `float_to_u32` specialised to canvas dimensions where we don't yet know
/// the canvas-px values themselves (the report prints `0` in that case).
fn mm_to_px_dim(v: f64, image_w: u32, image_h: u32) -> Result<u32, LayoutError> {
    float_to_u32(v, image_w, image_h, 0, 0)
}
