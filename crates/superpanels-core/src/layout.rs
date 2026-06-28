//! Canvas-as-truth layout. Each monitor is a rectangle in mm-space; the
//! image is a separate rectangle in the same mm-space. The crop for a
//! monitor is whatever piece of the image overlaps it, mapped from mm to
//! source-image pixels. Anything outside the image becomes black via the
//! `dst_offset` / `slice_dst_size` letterbox path.

use std::collections::HashMap;
use std::hash::BuildHasher;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::display::{Monitor, MonitorId, MonitorRef, Rotation};
use crate::schedule::{MonitorPlacement, monitor_key};

mod algorithm;

use algorithm::{EffectiveMonitor, validate_inputs, validate_monitors_physical};

/// Source-image pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The piece of the source image that lands on a single monitor, plus where
/// it lands inside that monitor's destination canvas. `dst_offset` +
/// `dst_size` describe the covered region; the rest is letterboxed black on
/// Apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CropSliceSpec {
    pub src_rect: Rect,
    pub dst_offset: (u32, u32),
    pub dst_size: (u32, u32),
}

/// Per-monitor crop and render parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct CropSpec {
    pub monitor_id: MonitorId,
    pub src_rect: Rect,
    #[serde(default)]
    pub dst_offset: (u32, u32),
    pub dst_size: (u32, u32),
    #[serde(default)]
    pub slice_dst_size: (u32, u32),
    pub rotation: Rotation,
}

impl CropSpec {
    #[must_use]
    pub fn needs_letterbox(&self) -> bool {
        self.dst_offset != (0, 0) || self.slice_dst_size != self.dst_size
    }
}

/// Image rectangle in canvas mm-space — the source of truth for span apply.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct ImageRectMm {
    pub x_mm: f32,
    pub y_mm: f32,
    pub w_mm: f32,
    pub h_mm: f32,
}

/// `FitMode` survives only as a per-monitor scaling hint for `PerMonitor`
/// profiles. Span profiles describe placement entirely via `ImageRectMm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    #[default]
    Fill,
    Fit,
    Stretch,
    Center,
}

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("monitor list cannot be empty")]
    EmptyMonitorList,

    #[error(
        "the following monitors are missing physical size; configure them with \
         `superpanels monitor configure <name>`: {monitors:?}"
    )]
    PhysicalSizeMissing { monitors: Vec<MonitorRef> },

    #[error("monitor `{name}` has invalid physical size (zero in one or both dimensions)")]
    InvalidPhysicalSize { name: String },

    #[error("image natural size must be non-zero, got {image_w}x{image_h}")]
    ImageZeroSize { image_w: u32, image_h: u32 },

    #[error("image rect mm must have positive width/height, got {w_mm}x{h_mm}")]
    ImageRectDegenerate { w_mm: f32, h_mm: f32 },

    /// A connected monitor has no entry in the profile's `monitor_state`.
    /// In normal use this is caught earlier by topology-fingerprint
    /// comparison; callers that bypass that gate (e.g. CLI `set --image`)
    /// should synthesise placements via [`synthesise_placements`].
    #[error("monitor `{name}` has no placement in profile.monitor_state")]
    PlacementMissing { name: String },
}

/// Compute one [`CropSpec`] per monitor — the canvas is the source of truth.
/// `image_rect_mm` is the image's rectangle in canvas mm-space; each
/// monitor's crop is the piece of the source image that maps onto its
/// `(x_mm, y_mm, w_mm, h_mm)` window. Out-of-image regions letterbox to
/// black via `dst_offset` / `slice_dst_size`.
pub fn compute_crop_specs<S: BuildHasher>(
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement, S>,
    image_size_px: (u32, u32),
    image_rect_mm: ImageRectMm,
) -> Result<Vec<CropSpec>, LayoutError> {
    validate_inputs(monitors, image_size_px)?;
    if !image_rect_mm.w_mm.is_finite()
        || !image_rect_mm.h_mm.is_finite()
        || image_rect_mm.w_mm <= 0.0
        || image_rect_mm.h_mm <= 0.0
    {
        return Err(LayoutError::ImageRectDegenerate {
            w_mm: image_rect_mm.w_mm,
            h_mm: image_rect_mm.h_mm,
        });
    }

    let mut specs = Vec::with_capacity(monitors.len());
    for m in monitors {
        let key = monitor_key(m);
        let Some(placement) = placements.get(&key) else {
            return Err(LayoutError::PlacementMissing {
                name: m.name.clone(),
            });
        };
        specs.push(crop_spec_for(m, *placement, image_size_px, image_rect_mm));
    }

    Ok(specs)
}

/// The crop one monitor takes from one image rectangle. Pure geometry —
/// callers validate `image_size_px` / `image_rect_mm` first. A monitor that
/// doesn't overlap the rectangle yields `slice_dst_size == (0, 0)` (the
/// caller letterboxes it black or, in a composite, skips the layer).
fn crop_spec_for(
    m: &Monitor,
    placement: MonitorPlacement,
    image_size_px: (u32, u32),
    image_rect_mm: ImageRectMm,
) -> CropSpec {
    let img_origin = (f64::from(image_rect_mm.x_mm), f64::from(image_rect_mm.y_mm));
    let px_per_mm = (
        f64::from(image_size_px.0) / f64::from(image_rect_mm.w_mm),
        f64::from(image_size_px.1) / f64::from(image_rect_mm.h_mm),
    );
    let eff = EffectiveMonitor::from_monitor(m, placement);
    let mon_origin = (f64::from(eff.placement.x_mm), f64::from(eff.placement.y_mm));
    let mon_size = (eff.width_mm, eff.height_mm);
    let mon_dst_size = (eff.pixel_w, eff.pixel_h);

    let unclamped = algorithm::UnclampedSrc {
        left: (mon_origin.0 - img_origin.0) * px_per_mm.0,
        top: (mon_origin.1 - img_origin.1) * px_per_mm.1,
        width: mon_size.0 * px_per_mm.0,
        height: mon_size.1 * px_per_mm.1,
    };
    let slice = algorithm::clamp_to_slice(&unclamped, image_size_px, mon_dst_size);

    CropSpec {
        monitor_id: m.id,
        src_rect: slice.src_rect,
        dst_offset: slice.dst_offset,
        dst_size: mon_dst_size,
        slice_dst_size: slice.dst_size,
        rotation: m.rotation,
    }
}

/// One monitor's composite plan: the destination framebuffer size plus every
/// image layer that overlaps it, in bottom-to-top render order. `slices` is
/// empty when no layer covers the monitor — it renders fully black.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorComposite {
    pub monitor_id: MonitorId,
    pub dst_size: (u32, u32),
    pub slices: Vec<MonitorSlice>,
}

/// A single layer's contribution to one monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorSlice {
    /// Index into the `layers` slice passed to [`compute_composite_crop_specs`].
    pub layer: usize,
    pub spec: CropSpec,
}

/// Composite variant of [`compute_crop_specs`]: each monitor collects a crop
/// from every overlapping layer instead of one image. `layers` is bottom-to-top
/// `(image_size_px, image_rect_mm)`; the apply pipeline alpha-stacks the slices
/// in that order. Non-overlapping layers are dropped per monitor.
pub fn compute_composite_crop_specs<S: BuildHasher>(
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement, S>,
    layers: &[((u32, u32), ImageRectMm)],
) -> Result<Vec<MonitorComposite>, LayoutError> {
    if monitors.is_empty() {
        return Err(LayoutError::EmptyMonitorList);
    }
    // Physical size is only needed to project a layer onto the canvas plane.
    // An empty composite paints every monitor black and projects nothing, so it
    // stays valid even on monitors that lack a configured physical size —
    // applying a blank canvas must not require monitor configuration.
    if !layers.is_empty() {
        validate_monitors_physical(monitors)?;
        for ((img_w, img_h), rect) in layers {
            if *img_w == 0 || *img_h == 0 {
                return Err(LayoutError::ImageZeroSize {
                    image_w: *img_w,
                    image_h: *img_h,
                });
            }
            if !rect.w_mm.is_finite()
                || !rect.h_mm.is_finite()
                || rect.w_mm <= 0.0
                || rect.h_mm <= 0.0
            {
                return Err(LayoutError::ImageRectDegenerate {
                    w_mm: rect.w_mm,
                    h_mm: rect.h_mm,
                });
            }
        }
    }

    let mut out = Vec::with_capacity(monitors.len());
    for m in monitors {
        let key = monitor_key(m);
        let Some(placement) = placements.get(&key) else {
            return Err(LayoutError::PlacementMissing {
                name: m.name.clone(),
            });
        };
        let eff = EffectiveMonitor::from_monitor(m, *placement);
        let dst_size = (eff.pixel_w, eff.pixel_h);
        let mut slices = Vec::new();
        for (idx, (size, rect)) in layers.iter().enumerate() {
            let spec = crop_spec_for(m, *placement, *size, *rect);
            if spec.slice_dst_size.0 == 0 || spec.slice_dst_size.1 == 0 {
                continue;
            }
            slices.push(MonitorSlice { layer: idx, spec });
        }
        out.push(MonitorComposite {
            monitor_id: m.id,
            dst_size,
            slices,
        });
    }
    Ok(out)
}

/// Pick a sensible default `ImageRectMm` that covers the monitor union with
/// the image's aspect preserved. Used by transient apply paths (CLI
/// `set --image`, daemon dry-run preview) that don't carry a canvas state.
#[must_use]
pub fn cover_image_rect_mm(monitors: &[Monitor], image_size_px: (u32, u32)) -> ImageRectMm {
    let placements = synthesise_placements(monitors);
    cover_image_rect_for(monitors, &placements, image_size_px)
}

/// [`cover_image_rect_mm`] over explicit placements: covers the bounding box
/// of the *placed* monitors (profile gaps included), aspect preserved. Used
/// for slideshow images without a per-image override, so the desktop matches
/// the GUI canvas's cover-fit seed.
#[must_use]
pub fn cover_image_rect_for<S: BuildHasher>(
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement, S>,
    image_size_px: (u32, u32),
) -> ImageRectMm {
    if monitors.is_empty() || image_size_px.0 == 0 || image_size_px.1 == 0 {
        return ImageRectMm::default();
    }
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for m in monitors {
        let key = monitor_key(m);
        let Some(p) = placements.get(&key) else {
            continue;
        };
        let eff = EffectiveMonitor::from_monitor(m, *p);
        #[allow(clippy::cast_possible_truncation)]
        // reason: f64→f32 narrowing of mm bounds is fine for a coarse cover-fit
        let (w, h) = (eff.width_mm as f32, eff.height_mm as f32);
        x0 = x0.min(p.x_mm);
        y0 = y0.min(p.y_mm);
        x1 = x1.max(p.x_mm + w);
        y1 = y1.max(p.y_mm + h);
    }
    if !x0.is_finite() || !y0.is_finite() {
        return ImageRectMm::default();
    }
    let bb_w = x1 - x0;
    let bb_h = y1 - y0;
    #[allow(clippy::cast_possible_truncation)]
    // reason: aspect ratio fits comfortably in f32 for any sane image
    let aspect = (f64::from(image_size_px.0) / f64::from(image_size_px.1)) as f32;
    let mut w = bb_w;
    let mut h = w / aspect;
    if h < bb_h {
        h = bb_h;
        w = h * aspect;
    }
    ImageRectMm {
        x_mm: x0 + (bb_w - w) / 2.0,
        y_mm: y0 + (bb_h - h) / 2.0,
        w_mm: w,
        h_mm: h,
    }
}

/// Synthesise placements from detected monitor positions. Used by the
/// transient apply paths (CLI `set --image`, daemon preview) that don't have
/// a profile in scope. Maps logical-pixel positions onto the mm plane via
/// each monitor's individual PPI.
#[must_use]
pub fn synthesise_placements(monitors: &[Monitor]) -> HashMap<String, MonitorPlacement> {
    let mut out = HashMap::with_capacity(monitors.len());
    for m in monitors {
        let key = monitor_key(m);
        let placement = synthesise_one(m);
        out.insert(key, placement);
    }
    out
}

#[allow(clippy::cast_possible_truncation)] // reason: f32 narrowing of mm values is fine for placement
fn synthesise_one(m: &Monitor) -> MonitorPlacement {
    let phys_mm = m.physical_size_mm.unwrap_or((1.0, 1.0));
    let (eff_w_mm, eff_w_px) = match m.rotation {
        Rotation::None | Rotation::Inverted => (phys_mm.0, m.resolution.0),
        Rotation::Left | Rotation::Right => (phys_mm.1, m.resolution.1),
    };
    let mm_per_px = if eff_w_px == 0 {
        0.0
    } else {
        eff_w_mm / f64::from(eff_w_px)
    };
    let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
    let x_mm = (f64::from(m.position.0) * scale * mm_per_px) as f32;
    let y_mm = (f64::from(m.position.1) * scale * mm_per_px) as f32;
    MonitorPlacement { x_mm, y_mm }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on errors; no recovery is meaningful
#[allow(clippy::expect_used)] // reason: tests fail loudly on errors; no recovery is meaningful
#[allow(clippy::panic)] // reason: panic on unexpected match shape is the test failure
mod tests {
    use super::*;
    use crate::display::MonitorId;

    #[allow(clippy::too_many_arguments)] // reason: a builder would obscure rather than clarify these tests
    fn monitor(
        id: u32,
        name: &str,
        w_px: u32,
        h_px: u32,
        x: i32,
        y: i32,
        w_mm: u32,
        h_mm: u32,
    ) -> Monitor {
        Monitor {
            id: MonitorId(id),
            name: name.to_owned(),
            stable_id: Some(format!("uuid-{id}")),
            position: (x, y),
            resolution: (w_px, h_px),
            physical_size_mm: Some((f64::from(w_mm), f64::from(h_mm))),
            scale: 1.0,
            rotation: Rotation::None,
            refresh_hz: None,
            ppi: None,
        }
    }

    fn place(x_mm: f32, y_mm: f32) -> MonitorPlacement {
        MonitorPlacement { x_mm, y_mm }
    }

    #[test]
    fn single_monitor_image_covering_returns_full_image() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 480.0,
            h_mm: 270.0,
        };
        let crops = compute_crop_specs(&monitors, &placements, (1920, 1080), rect).unwrap();
        assert_eq!(crops.len(), 1);
        assert_eq!(crops[0].src_rect.w, 1920);
        assert_eq!(crops[0].src_rect.h, 1080);
    }

    #[test]
    fn two_monitors_with_gap_skip_gap_in_source() {
        // Two monitors spaced 8 mm apart; the image covers both at 1:1 mm.
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 527, 296),
        ];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        placements.insert(monitor_key(&monitors[1]), place(527.0 + 8.0, 0.0));
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 527.0 + 8.0 + 527.0,
            h_mm: 296.0,
        };
        let crops = compute_crop_specs(&monitors, &placements, (7680, 1080), rect).unwrap();
        let m1_end = crops[0].src_rect.x + crops[0].src_rect.w;
        let gap = crops[1].src_rect.x.saturating_sub(m1_end);
        assert!(gap > 0, "expected non-zero gap, got {gap}");
        assert_eq!(crops[0].src_rect.w, crops[1].src_rect.w);
    }

    #[test]
    fn monitor_off_image_letterboxes_black() {
        // Monitor sits entirely above the image rect — its slice is empty.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, -1000.0));
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 480.0,
            h_mm: 270.0,
        };
        let crops = compute_crop_specs(&monitors, &placements, (1920, 1080), rect).unwrap();
        assert_eq!(crops[0].src_rect.w, 0);
        assert_eq!(crops[0].src_rect.h, 0);
        assert_eq!(crops[0].slice_dst_size, (0, 0));
        assert!(crops[0].needs_letterbox());
    }

    #[test]
    fn monitor_partly_off_image_clips_top() {
        // Monitor straddles the image's top edge: half the monitor is off.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, -135.0));
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 480.0,
            h_mm: 270.0,
        };
        let crops = compute_crop_specs(&monitors, &placements, (1920, 1080), rect).unwrap();
        assert_eq!(crops[0].src_rect.x, 0);
        assert_eq!(crops[0].src_rect.y, 0);
        assert_eq!(crops[0].src_rect.w, 1920);
        assert!(crops[0].src_rect.h <= 540 + 1 && crops[0].src_rect.h >= 540 - 1);
        assert!(crops[0].dst_offset.1 > 0);
        assert!(crops[0].needs_letterbox());
    }

    #[test]
    fn empty_monitor_list_returns_error() {
        let placements = HashMap::new();
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 100.0,
            h_mm: 100.0,
        };
        let result = compute_crop_specs(&[], &placements, (1920, 1080), rect);
        assert!(matches!(result, Err(LayoutError::EmptyMonitorList)));
    }

    #[test]
    fn missing_placement_returns_error() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let placements = HashMap::new();
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 480.0,
            h_mm: 270.0,
        };
        let result = compute_crop_specs(&monitors, &placements, (1920, 1080), rect);
        assert!(matches!(result, Err(LayoutError::PlacementMissing { .. })));
    }

    #[test]
    fn degenerate_image_rect_is_rejected() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 0.0,
            h_mm: 270.0,
        };
        let result = compute_crop_specs(&monitors, &placements, (1920, 1080), rect);
        assert!(matches!(
            result,
            Err(LayoutError::ImageRectDegenerate { .. })
        ));
    }

    #[test]
    fn cover_rect_for_placements_spans_profile_gaps() {
        // Two 527 mm panels with a 40 mm authored gap: the cover rect must
        // span the full 1094 mm placed width, not the detected 1054 mm.
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 527, 296),
        ];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        placements.insert(monitor_key(&monitors[1]), place(527.0 + 40.0, 0.0));

        let rect = cover_image_rect_for(&monitors, &placements, (3840, 2160));

        assert!((rect.w_mm - 1094.0).abs() < 0.5, "got w={}", rect.w_mm);
        // 16:9 at 1094 mm wide is ~615 mm tall — covers and overhangs the
        // 296 mm bounding box, centred on it.
        assert!(rect.h_mm > 296.0);
        assert!(rect.y_mm < 0.0);
    }

    #[test]
    fn synthesise_placements_round_trips_for_origin_monitor() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let placements = synthesise_placements(&monitors);
        let p = placements
            .get(&monitor_key(&monitors[0]))
            .expect("placement");
        assert!((p.x_mm - 0.0).abs() < 1e-3);
        assert!((p.y_mm - 0.0).abs() < 1e-3);
    }

    #[test]
    fn portrait_monitor_dst_size_uses_rotated_framebuffer() {
        // 1080×1920 panel rotated right (90 CW): the canvas-as-truth math
        // must hand back dst_size = (1080, 1920) — the framebuffer the
        // compositor draws into post-rotation.
        let landscape = Monitor {
            id: MonitorId(0),
            name: "DP-1".to_owned(),
            stable_id: Some("uuid-l".to_owned()),
            position: (0, 0),
            resolution: (1920, 1080),
            physical_size_mm: Some((527.0, 296.0)),
            scale: 1.0,
            rotation: Rotation::None,
            refresh_hz: None,
            ppi: None,
        };
        let portrait = Monitor {
            id: MonitorId(1),
            name: "DP-2".to_owned(),
            stable_id: Some("uuid-p".to_owned()),
            position: (1920, 0),
            resolution: (1920, 1080),
            physical_size_mm: Some((527.0, 296.0)),
            scale: 1.0,
            rotation: Rotation::Right,
            refresh_hz: None,
            ppi: None,
        };
        let monitors = vec![landscape, portrait];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        placements.insert(monitor_key(&monitors[1]), place(527.0, 0.0));
        let rect = ImageRectMm {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 527.0 + 296.0,
            h_mm: 527.0,
        };
        let crops = compute_crop_specs(&monitors, &placements, (3000, 1920), rect).unwrap();
        let portrait_crop = crops.iter().find(|c| c.monitor_id == MonitorId(1)).unwrap();
        assert_eq!(portrait_crop.dst_size, (1080, 1920));
        assert_eq!(portrait_crop.rotation, Rotation::Right);
        // Slice fully covers the rotated framebuffer — no letterbox margin —
        // so the apply pipeline can save `composed` as-is without a second
        // rotate (the property that justifies dropping rotate() in apply.rs).
        assert_eq!(portrait_crop.slice_dst_size, portrait_crop.dst_size);
        assert_eq!(portrait_crop.dst_offset, (0, 0));
        // The portrait monitor's mm width is 296 mm (its native short side
        // after rotation); at 3000 px / 823 mm ≈ 3.645 px/mm, that's roughly
        // 1079 px of source. Allow ±2 px for rounding.
        assert!(
            (1077..=1081).contains(&portrait_crop.src_rect.w),
            "expected ≈1079-px-wide src_rect, got {}",
            portrait_crop.src_rect.w
        );
    }

    #[test]
    fn fit_mode_serialises_as_snake_case() {
        assert_eq!(serde_json::to_string(&FitMode::Fill).unwrap(), "\"fill\"");
        assert_eq!(
            serde_json::to_string(&FitMode::Stretch).unwrap(),
            "\"stretch\""
        );
    }

    fn rect(x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32) -> ImageRectMm {
        ImageRectMm {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        }
    }

    #[test]
    fn composite_monitor_covered_by_one_layer_gets_one_slice() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        // Layer 0 covers the monitor; layer 1 sits far to the right.
        let layers = [
            ((1920, 1080), rect(0.0, 0.0, 480.0, 270.0)),
            ((800, 800), rect(2000.0, 0.0, 200.0, 200.0)),
        ];
        let out = compute_composite_crop_specs(&monitors, &placements, &layers).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dst_size, (1920, 1080));
        assert_eq!(out[0].slices.len(), 1);
        assert_eq!(out[0].slices[0].layer, 0);
    }

    #[test]
    fn composite_monitor_straddling_two_layers_keeps_both_in_order() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        // Both layers overlap the monitor (left half / right half).
        let layers = [
            ((1920, 1080), rect(0.0, 0.0, 240.0, 270.0)),
            ((1920, 1080), rect(240.0, 0.0, 240.0, 270.0)),
        ];
        let out = compute_composite_crop_specs(&monitors, &placements, &layers).unwrap();
        assert_eq!(out[0].slices.len(), 2);
        // Bottom-to-top order preserved.
        assert_eq!(out[0].slices[0].layer, 0);
        assert_eq!(out[0].slices[1].layer, 1);
        // Each only covers its half of the destination.
        assert!(out[0].slices[0].spec.needs_letterbox());
        assert!(out[0].slices[1].spec.needs_letterbox());
    }

    #[test]
    fn composite_monitor_covered_by_no_layer_has_empty_slices() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let layers = [((800, 800), rect(5000.0, 5000.0, 200.0, 200.0))];
        let out = compute_composite_crop_specs(&monitors, &placements, &layers).unwrap();
        assert!(out[0].slices.is_empty());
        // dst_size is still reported so the apply pipeline can render black.
        assert_eq!(out[0].dst_size, (1920, 1080));
    }

    #[test]
    fn composite_no_layers_yields_black_monitors() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let out = compute_composite_crop_specs(&monitors, &placements, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].slices.is_empty());
        assert_eq!(out[0].dst_size, (1920, 1080));
    }

    #[test]
    fn composite_missing_placement_returns_error() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let placements = HashMap::new();
        let layers = [((1920, 1080), rect(0.0, 0.0, 480.0, 270.0))];
        let result = compute_composite_crop_specs(&monitors, &placements, &layers);
        assert!(matches!(result, Err(LayoutError::PlacementMissing { .. })));
    }

    #[test]
    fn composite_no_layers_on_unconfigured_monitor_still_renders_black() {
        // An empty canvas projects nothing, so it must apply even when the
        // monitor has no configured physical size (M2).
        let mut m = monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270);
        m.physical_size_mm = None;
        let monitors = vec![m];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let out = compute_composite_crop_specs(&monitors, &placements, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].slices.is_empty());
        assert_eq!(out[0].dst_size, (1920, 1080));
    }

    #[test]
    fn composite_with_layers_on_unconfigured_monitor_errors() {
        // Projecting an actual layer still requires physical size.
        let mut m = monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270);
        m.physical_size_mm = None;
        let monitors = vec![m];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let layers = [((1920, 1080), rect(0.0, 0.0, 480.0, 270.0))];
        let result = compute_composite_crop_specs(&monitors, &placements, &layers);
        assert!(matches!(
            result,
            Err(LayoutError::PhysicalSizeMissing { .. })
        ));
    }
}
