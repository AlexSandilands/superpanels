//! Authored-placement-aware layout (`docs/spec/04-bezel-math.md`).
//!
//! Bezels and air-gaps are no longer a separate concept: each monitor's
//! `(x_mm, y_mm)` placement on the canvas is authored, and the gaps between
//! monitors fall out of those placements. The layout step consumes the
//! profile's `monitor_state` directly; live OS state is consulted only for
//! pixel resolutions and rotation.

use std::collections::HashMap;
use std::hash::BuildHasher;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::display::{Monitor, MonitorId, MonitorRef, Rotation};
use crate::schedule::{MonitorPlacement, monitor_key};

mod algorithm;

use algorithm::{EffectiveMonitor, SrcMapping, validate_inputs};

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
/// Apply (`docs/spec/04-bezel-math.md` §4.6).
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
    pub fit: FitMode,
}

impl CropSpec {
    #[must_use]
    pub fn needs_letterbox(&self) -> bool {
        self.dst_offset != (0, 0) || self.slice_dst_size != self.dst_size
    }
}

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

    #[error("image too small for canvas: {image_w}x{image_h} vs canvas {canvas_w}x{canvas_h}")]
    ImageTooSmall {
        image_w: u32,
        image_h: u32,
        canvas_w: u32,
        canvas_h: u32,
    },

    #[error("monitor `{name}` has invalid physical size (zero in one or both dimensions)")]
    InvalidPhysicalSize { name: String },

    #[error("fit mode `{mode:?}` is not yet implemented")]
    FitModeUnsupported { mode: FitMode },

    /// A connected monitor has no entry in the profile's `monitor_state`.
    /// In normal use this is caught earlier by topology-fingerprint
    /// comparison; callers that bypass that gate (e.g. CLI `set --image`)
    /// should synthesise placements via [`synthesise_placements`].
    #[error("monitor `{name}` has no placement in profile.monitor_state")]
    PlacementMissing { name: String },
}

/// Compute one [`CropSpec`] per monitor; the image is mapped onto the
/// authored placement plane in mm so crops form a continuous spanning
/// composition (`docs/spec/04-bezel-math.md`).
pub fn compute_crop_specs<S: BuildHasher>(
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement, S>,
    fit: FitMode,
    image_size: (u32, u32),
) -> Result<Vec<CropSpec>, LayoutError> {
    compute_crop_specs_with_offset(monitors, placements, fit, image_size, [0, 0], None)
}

#[allow(clippy::too_many_arguments)] // reason: free-positioning has 6 independent inputs; bundling reads worse
pub fn compute_crop_specs_with_offset<S: BuildHasher>(
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement, S>,
    fit: FitMode,
    image_size: (u32, u32),
    offset_px: [i32; 2],
    image_size_px: Option<[u32; 2]>,
) -> Result<Vec<CropSpec>, LayoutError> {
    validate_inputs(monitors, fit, image_size)?;

    let mut effs: Vec<EffectiveMonitor> = Vec::with_capacity(monitors.len());
    for m in monitors {
        let key = monitor_key(m);
        let Some(placement) = placements.get(&key) else {
            return Err(LayoutError::PlacementMissing {
                name: m.name.clone(),
            });
        };
        effs.push(EffectiveMonitor::from_monitor(m, *placement));
    }

    let reference_ppi = effs
        .iter()
        .map(EffectiveMonitor::ppi)
        .fold(0.0_f64, f64::max);

    let (canvas_width_mm, canvas_height_mm) = bounding_box_mm(&effs);
    let canvas = algorithm::compute_canvas_pixels(
        canvas_width_mm,
        canvas_height_mm,
        reference_ppi,
        image_size,
    )?;
    let mapping = SrcMapping::for_layout(fit, &canvas, image_size, image_size_px)?;

    let mut specs = Vec::with_capacity(monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        let eff = &effs[i];
        let mon_origin_mm = (f64::from(eff.placement.x_mm), f64::from(eff.placement.y_mm));
        let mon_size_mm = (eff.width_mm, eff.height_mm);
        let mon_dst_size = (eff.pixel_w, eff.pixel_h);
        let slice = mapping.monitor_to_slice(
            mon_origin_mm,
            mon_size_mm,
            mon_dst_size,
            reference_ppi,
            image_size,
            offset_px,
        );
        specs.push(CropSpec {
            monitor_id: m.id,
            src_rect: slice.src_rect,
            dst_offset: slice.dst_offset,
            dst_size: mon_dst_size,
            slice_dst_size: slice.dst_size,
            rotation: m.rotation,
            fit,
        });
    }

    Ok(specs)
}

fn bounding_box_mm(effs: &[EffectiveMonitor]) -> (f64, f64) {
    let mut max_w = 0.0_f64;
    let mut max_h = 0.0_f64;
    for e in effs {
        let r = f64::from(e.placement.x_mm) + e.width_mm;
        let b = f64::from(e.placement.y_mm) + e.height_mm;
        if r > max_w {
            max_w = r;
        }
        if b > max_h {
            max_h = b;
        }
    }
    (max_w, max_h)
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
    MonitorPlacement {
        x_mm,
        y_mm,
        rotation: m.rotation,
    }
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
            primary: false,
            ppi: None,
        }
    }

    fn place(x_mm: f32, y_mm: f32) -> MonitorPlacement {
        MonitorPlacement {
            x_mm,
            y_mm,
            rotation: Rotation::None,
        }
    }

    fn placements_from(monitors: &[Monitor]) -> HashMap<String, MonitorPlacement> {
        synthesise_placements(monitors)
    }

    #[test]
    fn single_monitor_no_gap_returns_full_image() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let placements = placements_from(&monitors);
        let crops =
            compute_crop_specs(&monitors, &placements, FitMode::Fill, (1920, 1080)).unwrap();
        assert_eq!(crops.len(), 1);
        assert_eq!(crops[0].src_rect.w, 1920);
        assert_eq!(crops[0].src_rect.h, 1080);
    }

    #[test]
    fn two_monitors_with_explicit_gap_skip_gap_in_source() {
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 527, 296),
        ];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        placements.insert(monitor_key(&monitors[1]), place(527.0 + 8.0, 0.0));
        let crops =
            compute_crop_specs(&monitors, &placements, FitMode::Fill, (7680, 1080)).unwrap();
        let m1_end = crops[0].src_rect.x + crops[0].src_rect.w;
        let gap = crops[1].src_rect.x.saturating_sub(m1_end);
        assert!(gap > 0, "expected non-zero gap, got {gap}");
        assert_eq!(crops[0].src_rect.w, crops[1].src_rect.w);
    }

    #[test]
    fn empty_monitor_list_returns_error() {
        let placements = HashMap::new();
        let result = compute_crop_specs(&[], &placements, FitMode::Fill, (1920, 1080));
        assert!(matches!(result, Err(LayoutError::EmptyMonitorList)));
    }

    #[test]
    fn missing_placement_returns_error() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let placements = HashMap::new();
        let result = compute_crop_specs(&monitors, &placements, FitMode::Fill, (1920, 1080));
        assert!(matches!(result, Err(LayoutError::PlacementMissing { .. })));
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
    fn fit_mode_serialises_as_snake_case() {
        assert_eq!(serde_json::to_string(&FitMode::Fill).unwrap(), "\"fill\"");
        assert_eq!(
            serde_json::to_string(&FitMode::Stretch).unwrap(),
            "\"stretch\""
        );
    }
}
