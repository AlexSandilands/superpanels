//! Bezel-aware layout data model.
//!
//! Defines the foundational types ([`BezelConfig`], [`Rect`], [`CropSpec`],
//! [`FitMode`]) that describe how a source image maps onto a multi-monitor
//! physical canvas. The `compute_crop_specs` algorithm (`SPEC.md` §4) and
//! `LayoutError` arrive in subsequent commits per the "First commits
//! playbook" in `PLAN.md`.

use serde::{Deserialize, Serialize};

use crate::display::{MonitorId, Rotation};

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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serde errors; no recovery is meaningful
mod tests {
    use super::*;

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
            fit: FitMode::Fit,
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
}
