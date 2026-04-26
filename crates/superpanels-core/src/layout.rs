//! Bezel-aware layout types and the [`compute_crop_specs`] entry point.
//!
//! Defines the foundational data types ([`BezelConfig`], [`Rect`], [`CropSpec`],
//! [`FitMode`]) that describe how a source image maps onto a multi-monitor
//! physical canvas, plus the [`compute_crop_specs`] algorithm (`SPEC.md` §4)
//! and its [`LayoutError`] type.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::display::{Monitor, MonitorId, MonitorRef, Rotation};

mod algorithm;

use algorithm::{
    EffectiveMonitor, SrcMapping, build_row_index, compute_canvas_layout, compute_canvas_pixels,
    group_into_rows, validate_inputs,
};

/// Physical gap sizes between adjacent screens, in millimetres.
///
/// Mirrors `SPEC.md` §3.2. The uniform `horizontal_mm` / `vertical_mm` pair
/// covers the typical setup where every adjacency uses the same bezel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BezelConfig {
    /// Uniform gap between any pair of horizontally adjacent monitors.
    pub horizontal_mm: f32,
    /// Uniform gap between any pair of vertically adjacent monitors.
    pub vertical_mm: f32,
}

/// An axis-aligned pixel rectangle within a source image.
///
/// Coordinates and dimensions are in source-image pixels (`SPEC.md` §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge, in source-image pixels.
    pub x: u32,
    /// Top edge, in source-image pixels.
    pub y: u32,
    /// Width, in source-image pixels.
    pub w: u32,
    /// Height, in source-image pixels.
    pub h: u32,
}

/// The slice of the source image that maps to one monitor, plus the
/// per-monitor render parameters needed to produce its temp file.
///
/// Mirrors `SPEC.md` §3.3. One [`CropSpec`] is produced per monitor by the
/// layout algorithm (`SPEC.md` §4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CropSpec {
    /// The runtime monitor this crop is for.
    pub monitor_id: MonitorId,
    /// Source-image rectangle to crop.
    pub src_rect: Rect,
    /// Target monitor pixel dimensions, post-rotation.
    pub dst_size: (u32, u32),
    /// Rotation applied during render so the saved file is right-side-up
    /// (see `SPEC.md` §4.3).
    pub rotation: Rotation,
    /// Fit mode used when choosing `src_rect`. Informational; useful to the
    /// GUI for reflecting the user's choice.
    pub fit: FitMode,
}

/// How a source image is fit to the physical desktop canvas.
///
/// Mirrors `SPEC.md` §3.4. The default is [`FitMode::Fill`], which matches
/// the assumption used in the `SPEC.md` §4 worked example.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FitMode {
    /// Cover the canvas, cropping any overflow.
    #[default]
    Fill,
    /// Fit the canvas, letterboxing any shortfall.
    Fit,
    /// Stretch to the canvas, ignoring aspect ratio.
    Stretch,
    /// Centre at native resolution, no scaling.
    Center,
}

/// Errors returned from the bezel-math entry point.
///
/// Mirrors `SPEC.md` §3 and §4.5. Each variant is something a caller can
/// surface specifically (the CLI prints a hint, the GUI shows a dedicated
/// toast) — flattening to a single string would lose that.
#[derive(Debug, Error)]
pub enum LayoutError {
    /// `compute_crop_specs` was called with an empty monitor slice.
    #[error("monitor list cannot be empty")]
    EmptyMonitorList,

    /// One or more monitors lack a configured `physical_size_mm`.
    ///
    /// `kscreen-doctor` and most compositor CLIs do not expose physical mm,
    /// so the user must declare it once via a `[[monitor]]` config block
    /// (see `SPEC.md` §14.1).
    #[error(
        "the following monitors are missing physical size; configure them with \
         `superpanels monitor configure <name>`: {monitors:?}"
    )]
    PhysicalSizeMissing {
        /// Monitors whose `physical_size_mm` is `None`.
        monitors: Vec<MonitorRef>,
    },

    /// The source image is too small for the canvas under the chosen fit mode.
    ///
    /// Also covers numeric out-of-range cases when mapping canvas mm to
    /// source-image pixels (e.g. floats that overflow `u32`); we fail loud
    /// rather than truncate silently.
    #[error("image too small for canvas: {image_w}x{image_h} vs canvas {canvas_w}x{canvas_h}")]
    ImageTooSmall {
        /// Source image width in pixels.
        image_w: u32,
        /// Source image height in pixels.
        image_h: u32,
        /// Computed canvas width in pixels.
        canvas_w: u32,
        /// Computed canvas height in pixels.
        canvas_h: u32,
    },

    /// A monitor declared `physical_size_mm` with a zero in either axis.
    #[error("monitor `{name}` has invalid physical size (zero in one or both dimensions)")]
    InvalidPhysicalSize {
        /// The offending monitor's `name` (e.g. `"DP-1"`).
        name: String,
    },

    /// The requested fit mode is not yet implemented.
    ///
    /// `Fill` and `Stretch` are implemented; `Fit` and `Center` defer to a
    /// later commit per the Phase 1.3 brief — the caller gets an explicit
    /// error rather than a silently miscomputed crop.
    #[error("fit mode `{mode:?}` is not yet implemented")]
    FitModeUnsupported {
        /// The mode the caller requested.
        mode: FitMode,
    },
}

/// Compute one [`CropSpec`] per monitor for the given image and bezel config.
///
/// The image is mapped onto the *physical* desktop plane in millimetres,
/// including bezel gaps, so the returned crops form a continuous spanning
/// composition when laid out across the actual screens (`SPEC.md` §4).
///
/// `image_size` is `(width, height)` in source-image pixels. `monitors` must
/// be non-empty and every monitor's `physical_size_mm` must be `Some`;
/// otherwise the function returns the corresponding [`LayoutError`].
///
/// # Errors
///
/// - [`LayoutError::EmptyMonitorList`] when `monitors` is empty.
/// - [`LayoutError::PhysicalSizeMissing`] when any monitor has
///   `physical_size_mm: None`.
/// - [`LayoutError::InvalidPhysicalSize`] when any monitor declares a zero
///   axis in `physical_size_mm`.
/// - [`LayoutError::FitModeUnsupported`] for [`FitMode::Fit`] or
///   [`FitMode::Center`] (deferred to a later commit).
/// - [`LayoutError::ImageTooSmall`] when source-image dimensions are zero or
///   when the mm→px math produces values that don't fit in `u32`.
pub fn compute_crop_specs(
    monitors: &[Monitor],
    bezels: &BezelConfig,
    fit: FitMode,
    image_size: (u32, u32),
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
    let mapping = SrcMapping::for_fit(fit, &canvas, image_size)?;

    let row_index_for_monitor = build_row_index(&rows, effs.len());
    let mut specs = Vec::with_capacity(monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        let mon_origin_mm = (
            layout.x_origin_mm[i],
            layout.row_y_mm[row_index_for_monitor[i]],
        );
        let mon_size_mm = (effs[i].width_mm, effs[i].height_mm);
        let src_rect = mapping.monitor_to_src_rect(
            mon_origin_mm,
            mon_size_mm,
            reference_ppi,
            image_size,
            (canvas.width, canvas.height),
        )?;
        specs.push(CropSpec {
            monitor_id: m.id,
            src_rect,
            dst_size: (effs[i].pixel_w, effs[i].pixel_h),
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
            physical_size_mm: Some((w_mm, h_mm)),
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
            dst_size: (1440, 2560),
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
        m.physical_size_mm = Some((0, 296));
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
                    physical_size_mm: Some((mw, mh)),
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
        }
    }
}
