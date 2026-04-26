//! Display detection data model and detector orchestration.
//!
//! Defines the foundational types ([`Monitor`], [`MonitorId`], [`Rotation`],
//! [`MonitorRef`]) that the rest of the core uses to talk about screens, plus
//! the [`DisplayDetector`] trait, [`Availability`] enum, [`DetectError`] enum,
//! and the [`detect`] orchestrator that walks detectors in priority order.
//!
//! Concrete detector implementations live in submodules ([`kscreen`] for
//! KDE Plasma, [`manual`] for the `--monitors` CLI override).

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod kscreen;
pub mod manual;

/// A physical display normalised into Superpanels' internal model.
///
/// Mirrors `SPEC.md` §3.1. Field ordering and semantics are part of the
/// spec; `Monitor` values are produced by detectors and consumed by the
/// layout module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    /// Runtime-only identity, assigned at detection time. Never persisted —
    /// use [`MonitorRef`] for data that must survive reboots and dock
    /// re-plugs.
    pub id: MonitorId,
    /// Compositor-supplied output name (e.g. `"DP-1"`, `"HDMI-A-1"`).
    pub name: String,
    /// Compositor-supplied stable identifier (KDE per-output UUID, or a
    /// hash of EDID `manufacturer + model + serial` on other backends).
    /// `None` when the detector cannot supply one.
    pub stable_id: Option<String>,
    /// Top-left corner in the logical desktop, in pixels (post-scale).
    pub position: (i32, i32),
    /// Pixel dimensions `(w, h)` in the monitor's native orientation.
    pub resolution: (u32, u32),
    /// Physical dimensions `(w, h)` in millimetres, native orientation.
    ///
    /// Sourced from per-monitor config (`SPEC.md` §14.1), **not** detection
    /// — `kscreen-doctor` and most compositor CLIs do not expose this.
    /// Remains `None` until the user has provided a `[[monitor]]` block;
    /// bezel math returns `LayoutError::PhysicalSizeMissing` while it is
    /// `None`.
    pub physical_size_mm: Option<(u32, u32)>,
    /// `HiDPI` scale factor (`1.0`, `1.25`, `1.5`, `2.0`, …).
    pub scale: f64,
    /// Display rotation applied by the compositor.
    pub rotation: Rotation,
    /// Refresh rate in Hz, for display in `superpanels detect` output. Not
    /// used by the layout math.
    pub refresh_hz: Option<f32>,
    /// Whether the compositor reports this monitor as primary.
    pub primary: bool,
    /// Pixels per inch, derived post-rotation from `resolution` and
    /// `physical_size_mm`. `None` whenever `physical_size_mm` is `None`.
    pub ppi: Option<f64>,
}

/// Newtype wrapping the runtime monitor identifier.
///
/// Distinct from `u32` so `fn apply(profile: ProfileId, monitor: MonitorId)`
/// cannot be called with the arguments swapped. Assigned during detection
/// and never persisted — see [`MonitorRef`] for the persistent counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(pub u32);

/// Display rotation applied by the compositor.
///
/// `None` is the default (matches `SPEC.md` §3.1, which uses `Rotation::None`
/// rather than `Normal`). Detector-specific numeric encodings (e.g. KDE's
/// `1`/`2`/`4`/`8` bitmask) are mapped to this enum by the parser, not baked
/// into the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Rotation {
    /// No rotation (landscape, the panel's native orientation).
    #[default]
    None,
    /// Rotated 90° clockwise.
    Right,
    /// Rotated 180°.
    Inverted,
    /// Rotated 90° counter-clockwise.
    Left,
}

/// Persistent reference to a monitor, stable across reboots and dock
/// re-plugs.
///
/// See `SPEC.md` §6.4. `stable_id` is a compositor-supplied identifier (KDE
/// per-output UUID, or a hash of EDID `manufacturer + model + serial`);
/// `name` (e.g. `"DP-1"`) is the fallback when the detector cannot supply a
/// stable id. Per-monitor config, profile assignments, and bezel overrides
/// all key on `MonitorRef` rather than the runtime [`MonitorId`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorRef {
    /// Compositor-supplied stable identifier.
    pub stable_id: String,
    /// Output name (e.g. `"DP-1"`); used as the human-readable fallback.
    pub name: String,
}

/// A source of [`Monitor`] data — typically a thin wrapper around a compositor
/// CLI tool (`kscreen-doctor`, `wlr-randr`, `xrandr`, …).
///
/// Mirrors `SPEC.md` §6.1. The orchestrator [`detect`] walks detectors in the
/// priority order defined by `SPEC.md` §6, calling [`Self::availability`]
/// first to skip detectors whose tool is missing or whose environment doesn't
/// match before paying the subprocess cost of [`Self::detect`].
pub trait DisplayDetector {
    /// Short, stable identifier (e.g. `"kscreen-doctor"`). Used by
    /// `superpanels detect --debug` to label per-detector output.
    fn name(&self) -> &str;
    /// Cheap, non-spawning check — env-var presence and `PATH` lookups only.
    fn availability(&self) -> Availability;
    /// Spawn the underlying tool and parse its output.
    fn detect(&self) -> Result<Vec<Monitor>, DetectError>;
}

/// Why a [`DisplayDetector`] is or isn't usable in the current environment.
///
/// Returned as an enum, not a `bool`, so `superpanels detect --debug` can
/// explain *why* each detector was skipped (`SPEC.md` §6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// Tool is present and the environment matches; [`DisplayDetector::detect`]
    /// is worth attempting.
    Available,
    /// The detector's underlying binary is not on `PATH`.
    ToolMissing {
        /// The binary the detector looked for (e.g. `"kscreen-doctor"`).
        tool: &'static str,
    },
    /// The detector's environment markers are absent (e.g.
    /// `$KDE_FULL_SESSION` not set for the KDE detector).
    WrongEnvironment {
        /// Human-readable reason suitable for `--debug` output.
        reason: &'static str,
    },
    /// The user pinned a different detector via config.
    Disabled,
}

/// Errors a [`DisplayDetector`] can return from [`DisplayDetector::detect`].
///
/// Distinct variants let the orchestrator react differently to "tool missing"
/// vs "tool present but failed" vs "parser broken" (`SPEC.md` §6.1).
#[derive(Debug, Error)]
pub enum DetectError {
    /// The subprocess exited non-zero or could not be spawned.
    #[error("subprocess `{cmd}` failed: {stderr}")]
    Subprocess {
        /// Command line for diagnostics (e.g. `"kscreen-doctor -o"`).
        cmd: String,
        /// Captured stderr (or `io::Error` description if the spawn itself
        /// failed).
        stderr: String,
    },
    /// The subprocess didn't return within the configured timeout.
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout {
        /// Command line for diagnostics.
        cmd: String,
        /// Configured timeout in seconds.
        seconds: u64,
    },
    /// The subprocess returned, but its output couldn't be parsed.
    #[error("could not parse output of `{cmd}`: {message}")]
    Parse {
        /// Command line for diagnostics.
        cmd: String,
        /// Parser-specific reason (line number, missing field, …).
        message: String,
    },
    /// The subprocess returned successfully but reported zero usable
    /// monitors. The orchestrator treats this as a soft failure and tries
    /// the next detector.
    #[error("detector returned an empty monitor list")]
    EmptyResult,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serde errors; no recovery is meaningful
mod tests {
    use super::*;

    fn sample_monitor(physical_size_mm: Option<(u32, u32)>) -> Monitor {
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
        let monitor = sample_monitor(Some((597, 336)));

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
