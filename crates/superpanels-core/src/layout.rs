//! Bezel-aware layout (`SPEC.md` §4).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::display::{Monitor, MonitorId, MonitorRef, Rotation};

mod algorithm;

use algorithm::{
    EffectiveMonitor, SrcMapping, build_row_index, compute_canvas_layout, compute_canvas_pixels,
    group_into_rows, validate_inputs,
};

/// Uniform horizontal/vertical bezel gaps in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BezelConfig {
    pub horizontal_mm: f32,
    pub vertical_mm: f32,
}

/// Source-image pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The piece of the source image that lands on a single monitor, plus where it
/// lands inside that monitor's destination canvas. `dst_offset` + `dst_size`
/// describe the covered region; the rest is letterboxed black on Apply
/// (`docs/spec/04-bezel-math.md` §4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CropSliceSpec {
    /// Source-image rectangle, clamped to `[0, image_w] × [0, image_h]`.
    pub src_rect: Rect,
    /// Top-left of the slice inside the monitor's destination plane.
    pub dst_offset: (u32, u32),
    /// Size of the covered region. Equals the monitor's `dst_size` when fully
    /// covered; smaller (or `(0, 0)`) when the source rect doesn't reach.
    pub dst_size: (u32, u32),
}

/// Per-monitor crop and render parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CropSpec {
    pub monitor_id: MonitorId,
    pub src_rect: Rect,
    /// Where the slice paints inside the monitor's full destination plane.
    /// `(0, 0)` for the legacy fully-covered path, non-zero only with
    /// letterboxed free-positioning (`docs/spec/04-bezel-math.md` §4.6).
    #[serde(default)]
    pub dst_offset: (u32, u32),
    /// Post-rotation pixel dimensions of the saved image — the full monitor
    /// canvas. The apply pipeline composes the (possibly smaller) painted
    /// slice onto a black canvas of this size.
    pub dst_size: (u32, u32),
    /// Size of the painted region inside `dst_size`. Equals `dst_size` for the
    /// fully-covered path; smaller when letterboxing applies. `(0, 0)` means
    /// the monitor is fully off-image and the apply pipeline writes pure
    /// black. Defaults to `dst_size` for backward-compatibility on read.
    #[serde(default)]
    pub slice_dst_size: (u32, u32),
    pub rotation: Rotation,
    pub fit: FitMode,
}

impl CropSpec {
    /// `true` when the slice doesn't fully cover the monitor — the apply
    /// pipeline must letterbox with black via `image::compose_on_black`.
    #[must_use]
    pub fn needs_letterbox(&self) -> bool {
        self.dst_offset != (0, 0) || self.slice_dst_size != self.dst_size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
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

    /// `kscreen-doctor` and most compositor CLIs do not expose physical mm,
    /// so the user must declare it once via a `[[monitor]]` config block.
    #[error(
        "the following monitors are missing physical size; configure them with \
         `superpanels monitor configure <name>`: {monitors:?}"
    )]
    PhysicalSizeMissing { monitors: Vec<MonitorRef> },

    /// Also covers mm→px math that overflows `u32` — we fail loud rather than
    /// truncate silently.
    #[error("image too small for canvas: {image_w}x{image_h} vs canvas {canvas_w}x{canvas_h}")]
    ImageTooSmall {
        image_w: u32,
        image_h: u32,
        canvas_w: u32,
        canvas_h: u32,
    },

    #[error("monitor `{name}` has invalid physical size (zero in one or both dimensions)")]
    InvalidPhysicalSize { name: String },

    /// `Fit` and `Center` are deferred (Phase 1.3); explicit error beats a
    /// silently miscomputed crop.
    #[error("fit mode `{mode:?}` is not yet implemented")]
    FitModeUnsupported { mode: FitMode },
}

/// Compute one [`CropSpec`] per monitor; the image is mapped onto the physical
/// desktop plane in mm including bezels, so crops form a continuous spanning
/// composition (`SPEC.md` §4). `image_size` is `(width, height)` in pixels.
pub fn compute_crop_specs(
    monitors: &[Monitor],
    bezels: &BezelConfig,
    fit: FitMode,
    image_size: (u32, u32),
) -> Result<Vec<CropSpec>, LayoutError> {
    compute_crop_specs_with_offset(monitors, bezels, fit, image_size, [0, 0], None)
}

/// Compute crop specs with a persisted image-position offset in physical-layout
/// canvas pixels (`SPEC §8`).
///
/// When `image_size_px` is `Some([w, h])`, the source rectangle on the canvas
/// is `(offset_px.x, offset_px.y, w, h)` regardless of `fit` — the GUI's free
/// transform overrides the FitMode-driven placement
/// (`docs/plan/phase-4c-free-positioning.md` §4c.2).
pub fn compute_crop_specs_with_offset(
    monitors: &[Monitor],
    bezels: &BezelConfig,
    fit: FitMode,
    image_size: (u32, u32),
    offset_px: [i32; 2],
    image_size_px: Option<[u32; 2]>,
) -> Result<Vec<CropSpec>, LayoutError> {
    validate_inputs(monitors, fit, image_size)?;

    let effs: Vec<EffectiveMonitor> = monitors
        .iter()
        .map(EffectiveMonitor::from_monitor)
        .collect();

    let reference_ppi = effs
        .iter()
        .map(EffectiveMonitor::ppi)
        .fold(0.0_f64, f64::max);

    let rows = group_into_rows(&effs);
    let layout = compute_canvas_layout(&effs, &rows, *bezels);
    let canvas = compute_canvas_pixels(&layout, reference_ppi, image_size)?;
    let mapping = SrcMapping::for_layout(fit, &canvas, image_size, image_size_px)?;

    let row_index_for_monitor = build_row_index(&rows, effs.len());
    let mut specs = Vec::with_capacity(monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        let mon_origin_mm = (
            layout.x_origin_mm[i],
            layout.row_y_mm[row_index_for_monitor[i]],
        );
        let mon_size_mm = (effs[i].width_mm, effs[i].height_mm);
        let mon_dst_size = (effs[i].pixel_w, effs[i].pixel_h);
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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on errors; no recovery is meaningful
#[allow(clippy::expect_used)] // reason: tests fail loudly on errors; no recovery is meaningful
#[allow(clippy::panic)] // reason: panic on unexpected match shape is the test failure
#[allow(clippy::cast_possible_truncation)] // reason: test helpers carefully bound their inputs
#[allow(clippy::cast_possible_wrap)] // reason: test helpers carefully bound their inputs
#[allow(clippy::cast_sign_loss)] // reason: test helpers carefully bound their inputs
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
            stable_id: None,
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

    fn zero_bezels() -> BezelConfig {
        BezelConfig {
            horizontal_mm: 0.0,
            vertical_mm: 0.0,
        }
    }

    #[test]
    fn bezel_config_round_trips_through_json() {
        // Arrange
        let bezels = BezelConfig {
            horizontal_mm: 8.0,
            vertical_mm: 5.0,
        };

        // Act
        let json = serde_json::to_string(&bezels).unwrap();
        let decoded: BezelConfig = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(decoded, bezels);
    }

    #[test]
    fn rect_round_trips_through_json() {
        // Arrange
        let rect = Rect {
            x: 10,
            y: 20,
            w: 1920,
            h: 1080,
        };

        // Act
        let json = serde_json::to_string(&rect).unwrap();
        let decoded: Rect = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(decoded, rect);
    }

    #[test]
    fn crop_spec_round_trips_through_json() {
        // Arrange
        let spec = CropSpec {
            monitor_id: MonitorId(2),
            src_rect: Rect {
                x: 100,
                y: 50,
                w: 1440,
                h: 2560,
            },
            dst_offset: (0, 0),
            dst_size: (1440, 2560),
            slice_dst_size: (1440, 2560),
            rotation: Rotation::Right,
            fit: FitMode::Stretch,
        };

        // Act
        let json = serde_json::to_string(&spec).unwrap();
        let decoded: CropSpec = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(decoded, spec);
    }

    #[test]
    fn fit_mode_serialises_as_snake_case() {
        // Locks the wire format. SPEC §14.1 + the GUI/IPC layer both expect
        // lowercase variants; reverting the `#[serde(rename_all)]` attribute
        // would break both and must trip this test.
        assert_eq!(serde_json::to_string(&FitMode::Fill).unwrap(), "\"fill\"");
        assert_eq!(serde_json::to_string(&FitMode::Fit).unwrap(), "\"fit\"");
        assert_eq!(
            serde_json::to_string(&FitMode::Stretch).unwrap(),
            "\"stretch\""
        );
        assert_eq!(
            serde_json::to_string(&FitMode::Center).unwrap(),
            "\"center\""
        );
    }

    #[test]
    fn fit_mode_rejects_pascal_case_on_deserialise() {
        // Companion to the snake_case lock-in: an old client sending the
        // PascalCase form must surface an error rather than silently
        // round-tripping back into the wrong variant.
        assert!(serde_json::from_str::<FitMode>("\"Fill\"").is_err());
        assert!(serde_json::from_str::<FitMode>("\"Stretch\"").is_err());
    }

    #[test]
    fn fit_mode_all_variants_round_trip_through_json() {
        // Arrange
        let variants = [
            FitMode::Fill,
            FitMode::Fit,
            FitMode::Stretch,
            FitMode::Center,
        ];

        // Act + Assert
        for fit in variants {
            let json = serde_json::to_string(&fit).unwrap();
            let decoded: FitMode = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, fit);
        }
    }

    #[test]
    fn fit_mode_default_is_fill() {
        // Arrange + Act
        let default = FitMode::default();

        // Assert
        assert_eq!(default, FitMode::Fill);
    }

    // -- compute_crop_specs --------------------------------------------------

    #[test]
    fn single_monitor_no_bezel_returns_full_image() {
        // Arrange — physical mm chosen at exact 16:9 (480×270) so that the
        // mm→px conversion lands on whole pixels and the assertion can be
        // exact. Real-hardware mm rounding is exercised by the proptests.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];

        // Act
        let crops =
            compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (1920, 1080)).unwrap();

        // Assert
        assert_eq!(crops.len(), 1);
        assert_eq!(crops[0].src_rect.x, 0);
        assert_eq!(crops[0].src_rect.y, 0);
        assert_eq!(crops[0].src_rect.w, 1920);
        assert_eq!(crops[0].src_rect.h, 1080);
        assert_eq!(crops[0].dst_size, (1920, 1080));
    }

    #[test]
    fn positive_offset_moves_crop_toward_source_origin() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];

        let centred =
            compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (3840, 1080)).unwrap();
        let shifted = compute_crop_specs_with_offset(
            &monitors,
            &zero_bezels(),
            FitMode::Fill,
            (3840, 1080),
            [100, 0],
            None,
        )
        .unwrap();

        assert!(shifted[0].src_rect.x < centred[0].src_rect.x);
        assert_eq!(shifted[0].src_rect.y, centred[0].src_rect.y);
    }

    #[test]
    fn out_of_bounds_offset_clamps_to_empty_slice() {
        // Phase 4c: drag offsets that push the source rect entirely off-image
        // produce empty slices (zero-area `Rect`, zero `dst_offset`/`dst_size`)
        // so the apply layer can letterbox the monitor with black instead of
        // failing with `ImageTooSmall` (`docs/plan/phase-4c-free-positioning.md`
        // §4c.2). The previous hard error broke any drag-then-Apply path.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];

        let crops = compute_crop_specs_with_offset(
            &monitors,
            &zero_bezels(),
            FitMode::Fill,
            (3840, 1080),
            [10_000, 0],
            None,
        )
        .expect("clamp must not return an error");

        assert_eq!(crops.len(), 1);
        let c = &crops[0];
        assert_eq!(c.src_rect.w, 0);
        assert_eq!(c.src_rect.h, 0);
        assert_eq!(c.dst_offset, (0, 0));
    }

    #[test]
    fn partial_offset_produces_letterbox_dst_offset() {
        // A drag that pushes the source's left edge past the canvas origin
        // partially uncovers the monitor — the crop algorithm must surface a
        // non-zero `dst_offset` so apply can paint black to the left of the
        // slice (`docs/plan/phase-4c-free-positioning.md` §4c.2).
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];

        let crops = compute_crop_specs_with_offset(
            &monitors,
            &zero_bezels(),
            FitMode::Fill,
            (1920, 1080),
            [200, 0],
            None,
        )
        .unwrap();

        assert!(crops[0].dst_offset.0 > 0, "expected letterbox on left edge");
        assert!(crops[0].src_rect.x == 0);
    }

    #[test]
    fn image_size_px_zoomed_2x_halves_src_rect() {
        // image_size_px = (2*image_w, 2*image_h) places the image at 2× scale
        // on the canvas, so the per-monitor source rect is half the size of
        // the no-zoom case. Locks in the §4c.2 contract that image_size_px
        // overrides FitMode-derived placement.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let image_size = (1920u32, 1080u32);

        let baseline = compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, image_size)
            .unwrap()[0]
            .src_rect;
        let zoomed = compute_crop_specs_with_offset(
            &monitors,
            &zero_bezels(),
            FitMode::Fill,
            image_size,
            [0, 0],
            Some([image_size.0 * 2, image_size.1 * 2]),
        )
        .unwrap()[0]
            .src_rect;

        // Within ±1 px to absorb rounding at canvas-px boundaries.
        let approx = |a: u32, b: u32| a.abs_diff(b) <= 1;
        assert!(
            approx(zoomed.w, baseline.w / 2),
            "zoomed.w {} should be ≈ baseline.w/2 {}",
            zoomed.w,
            baseline.w / 2
        );
        assert!(
            approx(zoomed.h, baseline.h / 2),
            "zoomed.h {} should be ≈ baseline.h/2 {}",
            zoomed.h,
            baseline.h / 2
        );
    }

    #[test]
    fn image_size_px_smaller_than_monitor_letterboxes() {
        // The user has shrunk the image to half the canvas width; the rest of
        // the monitor must letterbox.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270)];
        let image_size = (1920u32, 1080u32);
        // Place the image at canvas (0,0) with half-canvas size. The right
        // half of the monitor is uncovered.
        let crops = compute_crop_specs_with_offset(
            &monitors,
            &zero_bezels(),
            FitMode::Fill,
            image_size,
            [0, 0],
            Some([image_size.0 / 2, image_size.1]),
        )
        .unwrap();

        // src_rect covers the full image (we asked for exactly the image's
        // worth of canvas).
        assert!(crops[0].src_rect.w > 0);
        // The painted slice covers roughly half the monitor's dst width; the
        // right half is letterboxed (slice_dst_size.0 < dst_size.0).
        assert!(crops[0].slice_dst_size.0 < crops[0].dst_size.0);
        assert!(crops[0].needs_letterbox());
    }

    #[test]
    fn two_identical_monitors_zero_bezel_split_evenly() {
        // Arrange — exact-aspect mm, see `single_monitor_no_bezel_…` above.
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 480, 270),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 480, 270),
        ];

        // Act
        let crops =
            compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (3840, 1080)).unwrap();

        // Assert
        assert_eq!(crops.len(), 2);
        assert_eq!(crops[0].src_rect.w, 1920);
        assert_eq!(crops[1].src_rect.w, 1920);
        assert_eq!(crops[0].src_rect.x, 0);
        assert_eq!(crops[1].src_rect.x, 1920);
        // Monitors share a row so vertical extent equals the canvas.
        assert_eq!(crops[0].src_rect.h, 1080);
    }

    #[test]
    fn two_identical_monitors_uniform_bezel_skips_gap() {
        // Arrange — 8mm horizontal bezel between two identical 527mm panels.
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 527, 296),
        ];
        let bezels = BezelConfig {
            horizontal_mm: 8.0,
            vertical_mm: 0.0,
        };
        // Pick a wide canvas so Fill scales the image up cleanly.
        let image_size = (7680, 1080);

        // Act
        let crops = compute_crop_specs(&monitors, &bezels, FitMode::Fill, image_size).unwrap();

        // Assert — monitor 2 starts after monitor 1 + the bezel gap.
        let m1_end = crops[0].src_rect.x + crops[0].src_rect.w;
        let gap = crops[1].src_rect.x.saturating_sub(m1_end);
        assert!(
            gap > 0,
            "expected a non-zero gap between m1 and m2 src_rects, got {gap}"
        );
        // Crop widths should be equal (identical monitors).
        assert_eq!(crops[0].src_rect.w, crops[1].src_rect.w);
    }

    #[test]
    fn mixed_ppi_normalises_to_max_ppi() {
        // Arrange — 27" 2560×1440 (~108 PPI) + 24" 1920×1080 (~92 PPI),
        // side-by-side. After ref-PPI normalisation, src_rect widths are
        // proportional to physical mm (same source pixels per mm on both
        // monitors), so the image appears at the same physical scale on each
        // screen. SPEC §4.2.
        let monitors = vec![
            monitor(0, "DP-1", 2560, 1440, 0, 0, 597, 336),
            monitor(1, "DP-2", 1920, 1080, 2560, 0, 531, 299),
        ];

        // Act
        let crops =
            compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (8000, 1500)).unwrap();

        // Assert — source pixels per physical mm are equal across monitors.
        let pp_mm_0 = f64::from(crops[0].src_rect.w) / 597.0;
        let pp_mm_1 = f64::from(crops[1].src_rect.w) / 531.0;
        assert!(
            (pp_mm_0 - pp_mm_1).abs() < 0.05,
            "expected matching pixels-per-mm across normalised monitors (got {pp_mm_0} vs {pp_mm_1})"
        );
        // And the higher-PPI monitor still gets a higher native dst_size.
        assert!(crops[0].dst_size.0 > crops[1].dst_size.0);
    }

    #[test]
    fn portrait_monitor_uses_rotated_dimensions() {
        // Arrange — one landscape, one portrait (rotated Left). Portrait
        // monitor's effective width should equal its `physical_size_mm.1`.
        let landscape = monitor(0, "DP-1", 2560, 1440, 0, 0, 597, 336);
        let mut portrait = monitor(1, "DP-2", 2560, 1440, 2560, 0, 597, 336);
        portrait.rotation = Rotation::Left;

        let monitors = vec![landscape, portrait];

        // Act
        let crops =
            compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (8000, 2000)).unwrap();

        // Assert — landscape covers more horizontal mm than portrait.
        // Landscape eff_w_mm = 597; portrait eff_w_mm = 336. So landscape
        // crop should be wider in pixels at the same reference PPI.
        assert!(
            crops[0].src_rect.w > crops[1].src_rect.w,
            "expected landscape crop wider than portrait crop ({} vs {})",
            crops[0].src_rect.w,
            crops[1].src_rect.w
        );
        // Portrait dst_size is rotated.
        assert_eq!(crops[1].dst_size, (1440, 2560));
    }

    #[test]
    fn two_by_two_grid_with_uniform_bezels() {
        // Arrange — four identical monitors arranged 2x2.
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 527, 296),
            monitor(2, "DP-3", 1920, 1080, 0, 1080, 527, 296),
            monitor(3, "DP-4", 1920, 1080, 1920, 1080, 527, 296),
        ];
        let bezels = BezelConfig {
            horizontal_mm: 8.0,
            vertical_mm: 8.0,
        };

        // Act
        let crops = compute_crop_specs(&monitors, &bezels, FitMode::Fill, (7680, 4320)).unwrap();

        // Assert — top row's y is at 0 (or close), bottom row's y > top + monitor_h.
        assert_eq!(crops.len(), 4);
        assert_eq!(crops[0].src_rect.y, crops[1].src_rect.y);
        assert_eq!(crops[2].src_rect.y, crops[3].src_rect.y);
        assert!(crops[2].src_rect.y > crops[0].src_rect.y + crops[0].src_rect.h);
        // Vertical bezel respected: gap between top and bottom rows > 0.
        let top_end = crops[0].src_rect.y + crops[0].src_rect.h;
        assert!(crops[2].src_rect.y > top_end);
    }

    #[test]
    fn single_monitor_degenerate_canvas() {
        // Arrange — one 1920×1080 monitor and a *smaller* image. Fill should
        // scale up; we still produce a crop covering the full canvas.
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296)];

        // Act
        let crops = compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (1024, 768))
            .expect("Fill should scale the image up to cover a single-monitor canvas");

        // Assert — single crop, covering the whole (scaled) source image.
        assert_eq!(crops.len(), 1);
        assert_eq!(crops[0].src_rect.x, 0);
        assert_eq!(crops[0].src_rect.w, 1024);
    }

    #[test]
    fn empty_monitor_list_returns_error() {
        // Act
        let result = compute_crop_specs(&[], &zero_bezels(), FitMode::Fill, (1920, 1080));

        // Assert
        assert!(matches!(result, Err(LayoutError::EmptyMonitorList)));
    }

    #[test]
    fn monitor_with_no_physical_size_returns_physical_size_missing() {
        // Arrange
        let mut m = monitor(7, "HDMI-A-1", 1920, 1080, 0, 0, 527, 296);
        m.physical_size_mm = None;
        m.stable_id = Some("uuid-7".to_owned());
        let monitors = vec![m];

        // Act
        let result = compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (1920, 1080));

        // Assert
        let Err(LayoutError::PhysicalSizeMissing { monitors: refs }) = result else {
            panic!("expected PhysicalSizeMissing, got {result:?}");
        };
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "HDMI-A-1");
        assert_eq!(refs[0].stable_id, "uuid-7");
    }

    #[test]
    fn monitor_with_zero_physical_size_returns_invalid_physical_size() {
        // Arrange
        let mut m = monitor(0, "DP-1", 1920, 1080, 0, 0, 0, 296);
        m.physical_size_mm = Some((0.0, 296.0));
        let monitors = vec![m];

        // Act
        let result = compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fill, (1920, 1080));

        // Assert
        assert!(matches!(
            result,
            Err(LayoutError::InvalidPhysicalSize { ref name }) if name == "DP-1"
        ));
    }

    #[test]
    fn fit_mode_unsupported_returns_error_for_fit_and_center() {
        // Arrange
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296)];

        // Act + Assert — Fit
        let result = compute_crop_specs(&monitors, &zero_bezels(), FitMode::Fit, (1920, 1080));
        assert!(matches!(
            result,
            Err(LayoutError::FitModeUnsupported { mode: FitMode::Fit })
        ));

        // Act + Assert — Center
        let result = compute_crop_specs(&monitors, &zero_bezels(), FitMode::Center, (1920, 1080));
        assert!(matches!(
            result,
            Err(LayoutError::FitModeUnsupported {
                mode: FitMode::Center
            })
        ));
    }

    #[test]
    fn stretch_fills_canvas_independently_per_axis() {
        // Arrange — two monitors side-by-side; Stretch should map the full
        // image width across both, regardless of aspect.
        let monitors = vec![
            monitor(0, "DP-1", 1920, 1080, 0, 0, 527, 296),
            monitor(1, "DP-2", 1920, 1080, 1920, 0, 527, 296),
        ];

        // Act
        let crops =
            compute_crop_specs(&monitors, &zero_bezels(), FitMode::Stretch, (4000, 500)).unwrap();

        // Assert — sum of crop widths equals full image width (zero bezel).
        let total_w: u32 = crops.iter().map(|c| c.src_rect.w).sum();
        assert_eq!(total_w, 4000);
    }

    // -- property tests ------------------------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        // Strategy: 1-4 monitors arranged left-to-right in a single row,
        // with sane resolutions and physical sizes. A wide canvas (7680x2160)
        // ensures Fill always works.
        prop_compose! {
            fn arb_monitor(id: u32, x_offset: i32)
                          (rw in 1024u32..3840,
                           rh in 600u32..2160,
                           mw in 200u32..900,
                           mh in 150u32..600)
                          -> (Monitor, i32) {
                let m = Monitor {
                    id: MonitorId(id),
                    name: format!("DP-{id}"),
                    stable_id: None,
                    position: (x_offset, 0),
                    resolution: (rw, rh),
                    physical_size_mm: Some((f64::from(mw), f64::from(mh))),
                    scale: 1.0,
                    rotation: Rotation::None,
                    refresh_hz: None,
                    primary: false,
                    ppi: None,
                };
                let next_x = x_offset.saturating_add(rw as i32);
                (m, next_x)
            }
        }

        fn arb_monitors_with_mm() -> impl Strategy<Value = Vec<Monitor>> {
            (1usize..=4).prop_flat_map(|n| {
                let mut strategy: BoxedStrategy<Vec<Monitor>> = Just(Vec::new()).boxed();
                for i in 0..n {
                    strategy = (strategy, 0i32..1)
                        .prop_flat_map(move |(prev, _)| {
                            let next_x = prev.last().map_or(0, |m: &Monitor| {
                                m.position.0.saturating_add(m.resolution.0 as i32)
                            });
                            arb_monitor(i as u32, next_x).prop_map(move |(m, _)| {
                                let mut v = prev.clone();
                                v.push(m);
                                v
                            })
                        })
                        .boxed();
                }
                strategy
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig {
                cases: 64,
                .. ProptestConfig::default()
            })]

            #[test]
            fn every_monitor_receives_exactly_one_crop(monitors in arb_monitors_with_mm()) {
                let bezels = BezelConfig { horizontal_mm: 8.0, vertical_mm: 0.0 };
                let crops = compute_crop_specs(&monitors, &bezels, FitMode::Fill, (7680, 2160))
                    .expect("Fill on a wide canvas should always succeed");
                prop_assert_eq!(crops.len(), monitors.len());
                let monitor_ids: Vec<_> = monitors.iter().map(|m| m.id).collect();
                let crop_ids: Vec<_> = crops.iter().map(|c| c.monitor_id).collect();
                prop_assert_eq!(crop_ids, monitor_ids);
            }

            #[test]
            fn no_two_crops_overlap_in_source_image(monitors in arb_monitors_with_mm()) {
                let bezels = BezelConfig { horizontal_mm: 8.0, vertical_mm: 0.0 };
                let crops = compute_crop_specs(&monitors, &bezels, FitMode::Fill, (7680, 2160))
                    .expect("Fill on a wide canvas should always succeed");
                for (i, a) in crops.iter().enumerate() {
                    for b in crops.iter().skip(i + 1) {
                        let a_x_end = a.src_rect.x.saturating_add(a.src_rect.w);
                        let b_x_end = b.src_rect.x.saturating_add(b.src_rect.w);
                        let a_y_end = a.src_rect.y.saturating_add(a.src_rect.h);
                        let b_y_end = b.src_rect.y.saturating_add(b.src_rect.h);
                        let x_overlap = a.src_rect.x < b_x_end && b.src_rect.x < a_x_end;
                        let y_overlap = a.src_rect.y < b_y_end && b.src_rect.y < a_y_end;
                        prop_assert!(!(x_overlap && y_overlap),
                            "crops overlap: {:?} vs {:?}", a.src_rect, b.src_rect);
                    }
                }
            }

            #[test]
            fn sum_of_crop_widths_lte_image_width(monitors in arb_monitors_with_mm()) {
                let bezels = BezelConfig { horizontal_mm: 8.0, vertical_mm: 0.0 };
                let image_w = 7680u32;
                let crops = compute_crop_specs(&monitors, &bezels, FitMode::Fill, (image_w, 2160))
                    .expect("Fill on a wide canvas should always succeed");
                // Single-row layouts: crops are horizontally adjacent, so the
                // sum of their widths is bounded by the image width.
                let total: u64 = crops.iter().map(|c| u64::from(c.src_rect.w)).sum();
                prop_assert!(total <= u64::from(image_w),
                    "sum {total} exceeded image width {image_w}");
            }

            #[test]
            fn small_inbounds_offset_preserves_one_crop_per_monitor_and_no_overlap(
                monitors in arb_monitors_with_mm(),
                ox in -200i32..=200,
                oy in -200i32..=200,
            ) {
                // The offset range is bounded so that the resulting `src_left`
                // is guaranteed to stay inside the source image — the contract
                // we want to lock in is that *legal* drags don't break the
                // per-monitor invariants from `every_monitor_receives_*` and
                // `no_two_crops_overlap_in_source_image`.
                let bezels = BezelConfig { horizontal_mm: 8.0, vertical_mm: 0.0 };
                let image_size = (7680u32, 2160u32);
                let Ok(crops) = compute_crop_specs_with_offset(
                    &monitors, &bezels, FitMode::Fill, image_size, [ox, oy], None,
                ) else {
                    // The clamp path is total but the validation path can still
                    // reject canvases that overflow `u32` for unusual mm sizes;
                    // skip those, the invariants below are about *successful*
                    // drags.
                    return Ok(());
                };

                prop_assert_eq!(crops.len(), monitors.len());
                let monitor_ids: Vec<_> = monitors.iter().map(|m| m.id).collect();
                let crop_ids: Vec<_> = crops.iter().map(|c| c.monitor_id).collect();
                prop_assert_eq!(crop_ids, monitor_ids);

                for (i, a) in crops.iter().enumerate() {
                    for b in crops.iter().skip(i + 1) {
                        let a_x_end = a.src_rect.x.saturating_add(a.src_rect.w);
                        let b_x_end = b.src_rect.x.saturating_add(b.src_rect.w);
                        let a_y_end = a.src_rect.y.saturating_add(a.src_rect.h);
                        let b_y_end = b.src_rect.y.saturating_add(b.src_rect.h);
                        let x_overlap = a.src_rect.x < b_x_end && b.src_rect.x < a_x_end;
                        let y_overlap = a.src_rect.y < b_y_end && b.src_rect.y < a_y_end;
                        prop_assert!(!(x_overlap && y_overlap),
                            "offset [{ox},{oy}] produced overlapping crops: {:?} vs {:?}",
                            a.src_rect, b.src_rect);
                    }
                }
            }

            #[test]
            fn any_offset_clamps_src_rect_inside_image_and_letterbox_inside_monitor(
                monitors in arb_monitors_with_mm(),
                ox in -50_000i32..=50_000,
                oy in -50_000i32..=50_000,
            ) {
                // Phase 4c §4c.2 invariants: `src_rect` always sits inside
                // `(0, 0, image_w, image_h)`, and `dst_offset + dst_size`
                // never overflows the monitor's `dst_size`.
                let bezels = BezelConfig { horizontal_mm: 8.0, vertical_mm: 0.0 };
                let image_size = (7680u32, 2160u32);
                let Ok(crops) = compute_crop_specs_with_offset(
                    &monitors, &bezels, FitMode::Fill, image_size, [ox, oy], None,
                ) else {
                    return Ok(());
                };
                for c in &crops {
                    let x_end = u64::from(c.src_rect.x) + u64::from(c.src_rect.w);
                    let y_end = u64::from(c.src_rect.y) + u64::from(c.src_rect.h);
                    prop_assert!(x_end <= u64::from(image_size.0),
                        "src_rect overflows image_w: {:?}", c.src_rect);
                    prop_assert!(y_end <= u64::from(image_size.1),
                        "src_rect overflows image_h: {:?}", c.src_rect);
                    // The painted slice (dst_offset + slice_dst_size) must
                    // fit inside the monitor's full dst (`dst_size`).
                    let dx_end = u64::from(c.dst_offset.0) + u64::from(c.slice_dst_size.0);
                    let dy_end = u64::from(c.dst_offset.1) + u64::from(c.slice_dst_size.1);
                    prop_assert!(dx_end <= u64::from(c.dst_size.0),
                        "slice dst overflow x: dst_offset={:?} slice_dst_size={:?} dst_size={:?}",
                        c.dst_offset, c.slice_dst_size, c.dst_size);
                    prop_assert!(dy_end <= u64::from(c.dst_size.1),
                        "slice dst overflow y: dst_offset={:?} slice_dst_size={:?} dst_size={:?}",
                        c.dst_offset, c.slice_dst_size, c.dst_size);
                }
            }
        }
    }
}
