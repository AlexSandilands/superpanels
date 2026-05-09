//! Display detection (`SPEC.md` §6).

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod hyprctl;
pub mod kscreen;
pub mod manual;
mod stable_id;
mod subprocess;
pub mod wlr_randr;
pub mod xrandr;

use hyprctl::HyprctlDetector;
use kscreen::KscreenDoctorDetector;
use manual::parse_manual_monitors;
pub(crate) use subprocess::{run as run_subprocess, which};
use wlr_randr::WlrRandrDetector;
use xrandr::XrandrDetector;

/// A physical display normalised into Superpanels' internal model
/// (`SPEC.md` §3.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    /// Runtime-only; never persisted — use [`MonitorRef`] for that.
    pub id: MonitorId,
    pub name: String,
    /// KDE per-output UUID, or hash of EDID `manufacturer + model + serial`.
    pub stable_id: Option<String>,
    /// Top-left in logical desktop pixels (post-scale).
    pub position: (i32, i32),
    /// Native-orientation pixel dimensions.
    pub resolution: (u32, u32),
    /// Native-orientation mm. Sourced from `[[monitor]]` config — detectors
    /// don't expose this. `None` until the user fills it in; bezel math
    /// returns `LayoutError::PhysicalSizeMissing` until then.
    pub physical_size_mm: Option<(f64, f64)>,
    pub scale: f64,
    pub rotation: Rotation,
    pub refresh_hz: Option<f32>,
    pub primary: bool,
    /// Post-rotation, derived from `resolution` and `physical_size_mm`.
    pub ppi: Option<f64>,
}

/// Runtime monitor id. Newtype so `fn apply(profile: ProfileId, monitor: MonitorId)`
/// can't be called with arguments swapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    #[default]
    None,
    Right,
    Inverted,
    Left,
}

/// Persistent monitor reference (`SPEC.md` §6.4). All persisted data —
/// per-monitor config, profile assignments, bezel overrides — keys on this
/// rather than the runtime [`MonitorId`]. `name` is the fallback when the
/// detector can't supply a `stable_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorRef {
    pub stable_id: String,
    pub name: String,
}

/// A source of [`Monitor`] data — usually a thin wrapper around a compositor
/// CLI. [`detect`] walks implementations in `SPEC.md` §6 priority order.
pub trait DisplayDetector {
    fn name(&self) -> &str;
    /// Cheap check — env vars and `PATH` only, no subprocess spawn.
    fn availability(&self) -> Availability;
    fn detect(&self) -> Result<Vec<Monitor>, DetectError>;
}

/// Enum (not bool) so `detect --debug` can explain *why* each detector was
/// skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    ToolMissing { tool: &'static str },
    WrongEnvironment { reason: &'static str },
    Disabled,
}

#[derive(Debug, Error)]
pub enum DetectError {
    #[error("subprocess `{cmd}` failed: {stderr}")]
    Subprocess { cmd: String, stderr: String },
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout { cmd: String, seconds: u64 },
    #[error("could not parse output of `{cmd}`: {message}")]
    Parse { cmd: String, message: String },
    /// Soft failure — the orchestrator falls through to the next detector.
    #[error("detector returned an empty monitor list")]
    EmptyResult,
}

/// Try detectors in `SPEC.md` §6 priority order; return the first non-empty
/// layout. `manual_override` (`--monitors`) wins unconditionally. Returned
/// monitors have `physical_size_mm: None`; merging against `[[monitor]]`
/// config happens in Phase 1.6.
pub fn detect(manual_override: Option<&str>) -> Result<Vec<Monitor>, DetectError> {
    if let Some(spec) = manual_override {
        return parse_manual_monitors(spec);
    }

    let detectors: [&dyn DisplayDetector; 4] = [
        &KscreenDoctorDetector,
        &HyprctlDetector,
        &WlrRandrDetector,
        &XrandrDetector,
    ];

    for detector in detectors {
        if detector.availability() != Availability::Available {
            continue;
        }
        if let Ok(monitors) = detector.detect()
            && !monitors.is_empty()
        {
            return Ok(monitors);
        }
    }

    Err(DetectError::Subprocess {
        cmd: "all detectors".to_owned(),
        stderr: "Could not detect monitor layout. Try --monitors WxH+X+Y,... \
                 to specify manually, or run 'superpanels detect --debug' \
                 to see what was attempted."
            .to_owned(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serde errors; no recovery is meaningful
mod tests {
    use super::*;

    fn sample_monitor(physical_size_mm: Option<(f64, f64)>) -> Monitor {
        Monitor {
            id: MonitorId(0),
            name: "DP-1".to_owned(),
            stable_id: Some("f7f0f124-9e9b-4ef0-91a7-426d58091760".to_owned()),
            position: (0, 0),
            resolution: (2560, 1440),
            physical_size_mm,
            scale: 1.0,
            rotation: Rotation::None,
            refresh_hz: Some(60.0),
            primary: true,
            ppi: physical_size_mm.map(|_| 108.79),
        }
    }

    #[test]
    fn monitor_with_physical_size_round_trips_through_json() {
        // Arrange
        let monitor = sample_monitor(Some((597.0, 336.0)));

        // Act
        let json = serde_json::to_string(&monitor).unwrap();
        let decoded: Monitor = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(decoded, monitor);
    }

    #[test]
    fn monitor_without_physical_size_round_trips_through_json() {
        // Arrange
        let monitor = sample_monitor(None);

        // Act
        let json = serde_json::to_string(&monitor).unwrap();
        let decoded: Monitor = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(decoded, monitor);
        assert_eq!(decoded.physical_size_mm, None);
        assert_eq!(decoded.ppi, None);
    }

    #[test]
    fn rotation_serialises_as_snake_case() {
        // Locks the wire format. The frontend (`ui/src/lib/api.ts`) and the
        // canvas layout module both assume lowercase variants; reverting the
        // `#[serde(rename_all)]` attribute would break monitor detection
        // round-trips silently and must trip this test.
        assert_eq!(serde_json::to_string(&Rotation::None).unwrap(), "\"none\"");
        assert_eq!(
            serde_json::to_string(&Rotation::Right).unwrap(),
            "\"right\""
        );
        assert_eq!(
            serde_json::to_string(&Rotation::Inverted).unwrap(),
            "\"inverted\""
        );
        assert_eq!(serde_json::to_string(&Rotation::Left).unwrap(), "\"left\"");
    }

    #[test]
    fn rotation_rejects_pascal_case_on_deserialise() {
        // Companion to the snake_case lock-in: an old client sending the
        // PascalCase form must fail loudly rather than fall back to default.
        assert!(serde_json::from_str::<Rotation>("\"None\"").is_err());
        assert!(serde_json::from_str::<Rotation>("\"Left\"").is_err());
    }

    #[test]
    fn rotation_all_variants_round_trip_through_json() {
        // Arrange
        let variants = [
            Rotation::None,
            Rotation::Right,
            Rotation::Inverted,
            Rotation::Left,
        ];

        // Act + Assert
        for rotation in variants {
            let json = serde_json::to_string(&rotation).unwrap();
            let decoded: Rotation = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, rotation);
        }
    }

    #[test]
    fn monitor_ref_round_trips_through_json() {
        // Arrange
        let monitor_ref = MonitorRef {
            stable_id: "f7f0f124-9e9b-4ef0-91a7-426d58091760".to_owned(),
            name: "DP-1".to_owned(),
        };

        // Act
        let json = serde_json::to_string(&monitor_ref).unwrap();
        let decoded: MonitorRef = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(decoded, monitor_ref);
    }
}
