//! Display detection data model.
//!
//! Defines the foundational types ([`Monitor`], [`MonitorId`], [`Rotation`],
//! [`MonitorRef`]) that the rest of the core uses to talk about screens.
//! Concrete detector implementations (kscreen-doctor, wlr-randr, hyprctl,
//! xrandr) and the [`DisplayDetector`] trait land in later commits per the
//! "First commits playbook" in `PLAN.md`.

use serde::{Deserialize, Serialize};

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
