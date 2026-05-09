//! Top-level schedule rules and shared time/profile-state types
//! (`docs/spec/09-profiles-schedules.md`).
//!
//! Schedules are independent of profiles: each rule names a profile by string
//! and fires on a clock trigger. `LatLong` is needed for sunset/sunrise
//! triggers. `MonitorPlacement`, `TopologyFingerprint`, and `ProfileColour`
//! also live here because they're referenced from both `Profile` and the
//! validity machinery.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use ts_rs::TS;

use crate::display::{Monitor, Rotation};

/// Curated 12-swatch palette used for tray pills, manager rows, and dialog
/// pickers. Values map 1:1 to the mockup's `SWATCHES` list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(rename_all = "snake_case")]
pub enum ProfileColour {
    #[default]
    Slate,
    Stone,
    Red,
    Orange,
    Amber,
    Yellow,
    Lime,
    Emerald,
    Teal,
    Sky,
    Indigo,
    Violet,
}

impl ProfileColour {
    /// Stable palette order, used by the GUI's "next unused swatch" picker
    /// and by tests that pin the palette length.
    pub const ALL: [Self; 12] = [
        Self::Slate,
        Self::Stone,
        Self::Red,
        Self::Orange,
        Self::Amber,
        Self::Yellow,
        Self::Lime,
        Self::Emerald,
        Self::Teal,
        Self::Sky,
        Self::Indigo,
        Self::Violet,
    ];
}

/// Authored monitor canvas state lifted out of the transient
/// `MonitorOverride` from `ui/src/lib/stores/canvas-view.svelte.ts`. The
/// `(x_mm, y_mm)` are the monitor's top-left in physical millimetres on the
/// authored canvas; gaps between monitors fall out of the placements.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct MonitorPlacement {
    pub x_mm: f32,
    pub y_mm: f32,
    pub rotation: Rotation,
}

/// Opaque hash over (sorted `stable_id`s ∥ rotations) for the connected set
/// at apply time. Profile authoring captures it; topology comparison is by
/// equality. Pixel resolution is deliberately excluded — a resolution change
/// rescales gracefully, only orientation flips invalidate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct TopologyFingerprint(pub String);

impl TopologyFingerprint {
    /// Hash from a slice of detected monitors. Stable across processes:
    /// sorts by `stable_id` then hashes `<id>:<rotation>` joined with `|`.
    #[must_use]
    pub fn from_monitors(monitors: &[Monitor]) -> Self {
        let mut entries: BTreeMap<String, Rotation> = BTreeMap::new();
        for m in monitors {
            let id = monitor_key(m);
            entries.insert(id, m.rotation);
        }
        let mut hasher = Sha256::new();
        for (i, (id, rot)) in entries.iter().enumerate() {
            if i > 0 {
                hasher.update(b"|");
            }
            hasher.update(id.as_bytes());
            hasher.update(b":");
            hasher.update(rotation_tag(*rot).as_bytes());
        }
        let digest = hasher.finalize();
        Self(hex(&digest))
    }
}

/// Canonical key used by `monitor_state` (`Monitor.stable_id` if present,
/// otherwise the `name`). Mirrors the existing `MonitorRef` fallback so the
/// fingerprint is consistent with persistent monitor references.
#[must_use]
pub fn monitor_key(m: &Monitor) -> String {
    m.stable_id.clone().unwrap_or_else(|| m.name.clone())
}

fn rotation_tag(r: Rotation) -> &'static str {
    match r {
        Rotation::None => "n",
        Rotation::Right => "r",
        Rotation::Inverted => "i",
        Rotation::Left => "l",
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(HEX[usize::from(b >> 4)]));
        out.push(char::from(HEX[usize::from(b & 0x0f)]));
    }
    out
}

/// Decimal degrees lat/long for sunset/sunrise scheduling
/// (`docs/spec/09-profiles-schedules.md` §9.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct LatLong {
    pub lat: f64,
    pub lon: f64,
}

/// Trigger half of a [`Schedule`]. Promoted out of profile bodies so a
/// rule can reference any profile by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Daily {
        hour: u8,
        minute: u8,
    },
    /// Sunset / sunrise with an offset in minutes (negative = before, positive = after).
    Sunset {
        offset_minutes: i32,
    },
    Sunrise {
        offset_minutes: i32,
    },
    /// Cron expression validated at the IPC boundary against the `cron` crate.
    Cron {
        expr: String,
    },
}

/// Top-level schedule rule. References a profile by name; bound at fire time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
pub struct Schedule {
    /// Optional human-readable label (for the manager UI). `None` falls back
    /// to a derived summary like "Daily 18:00 → dark".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub profile: String,
    pub trigger: Trigger,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("invalid cron expression `{expr}`: {message}")]
    InvalidCron { expr: String, message: String },
    #[error("daily trigger out of range: hour={hour} minute={minute}")]
    InvalidDaily { hour: u8, minute: u8 },
    #[error("two schedule rules collide at the same minute: `{a}` and `{b}` both fire at {when}")]
    SameMinuteCollision { a: String, b: String, when: String },
    #[error("schedule references unknown profile `{name}`")]
    UnknownProfile { name: String },
    #[error("sunset/sunrise rule requires `location` in config")]
    LocationMissing,
}

/// Validate one rule's trigger shape (range, cron parseability). Conflict
/// detection across rules lives separately in `detect_same_minute_collision`.
pub fn validate_trigger(t: &Trigger) -> Result<(), ScheduleError> {
    match t {
        Trigger::Daily { hour, minute } => {
            if *hour > 23 || *minute > 59 {
                Err(ScheduleError::InvalidDaily {
                    hour: *hour,
                    minute: *minute,
                })
            } else {
                Ok(())
            }
        }
        Trigger::Sunset { .. } | Trigger::Sunrise { .. } => Ok(()),
        Trigger::Cron { expr } => {
            use std::str::FromStr as _;
            cron::Schedule::from_str(expr)
                .map(|_| ())
                .map_err(|e| ScheduleError::InvalidCron {
                    expr: expr.clone(),
                    message: e.to_string(),
                })
        }
    }
}

/// Hand-rolled sunrise/sunset approximation per Almanac §"Sunrise equation"
/// — accurate to ~±2 minutes at temperate latitudes, degrading at high
/// latitudes near the polar day/night cutoff. `kind` selects sunrise vs
/// sunset; `date_utc_days` is days since 1970-01-01 (`chrono::NaiveDate`).
/// Returns minutes after UTC midnight on that date, or `None` if the sun
/// doesn't rise/set on that date at that latitude.
#[must_use]
#[allow(clippy::cast_precision_loss)] // reason: jd minute is ±2 min already
pub fn sun_event_utc_minutes(
    location: LatLong,
    date_utc_days: i64,
    is_sunrise: bool,
) -> Option<i32> {
    // Reference: en.wikipedia.org/wiki/Sunrise_equation, Almanac approximation.
    // Inputs are radians for trig math; we work in degrees externally.
    let n_days = date_utc_days as f64 - (date_utc_days as f64 / 36_525.0).floor() + 2_440_588.0
        - 2_451_545.0
        + 0.0008;
    let j_star = n_days - location.lon / 360.0;
    let mean_anomaly_deg = (357.5291 + 0.985_600_28 * j_star) % 360.0;
    let m = mean_anomaly_deg.to_radians();
    let centre = 1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.000_3 * (3.0 * m).sin();
    let lambda_deg = (mean_anomaly_deg + centre + 180.0 + 102.9372) % 360.0;
    let lambda = lambda_deg.to_radians();
    let j_transit = 2_451_545.0 + j_star + 0.0053 * m.sin() - 0.0069 * (2.0 * lambda).sin();
    let sin_decl = lambda.sin() * 23.44_f64.to_radians().sin();
    let cos_decl = (1.0 - sin_decl * sin_decl).sqrt();
    let lat = location.lat.to_radians();
    let cos_h = ((-0.83_f64.to_radians()).sin() - lat.sin() * sin_decl) / (lat.cos() * cos_decl);
    if !cos_h.is_finite() || !(-1.0..=1.0).contains(&cos_h) {
        return None;
    }
    let h_deg = cos_h.acos().to_degrees();
    let event_jd = if is_sunrise {
        j_transit - h_deg / 360.0
    } else {
        j_transit + h_deg / 360.0
    };
    let date_jd_midnight = 2_440_587.5 + date_utc_days as f64;
    let minutes = ((event_jd - date_jd_midnight) * 24.0 * 60.0).round();
    if !minutes.is_finite() || !(-1_440.0..=2_880.0).contains(&minutes) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)] // reason: bounded above
    Some(minutes as i32)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serde or hash errors
mod tests {
    use super::*;
    use crate::display::{Monitor, MonitorId};

    fn mon(id: u32, stable: Option<&str>, name: &str, rot: Rotation) -> Monitor {
        Monitor {
            id: MonitorId(id),
            name: name.to_owned(),
            stable_id: stable.map(str::to_owned),
            position: (0, 0),
            resolution: (1920, 1080),
            physical_size_mm: Some((527.0, 296.0)),
            scale: 1.0,
            rotation: rot,
            refresh_hz: None,
            primary: true,
            ppi: None,
        }
    }

    #[test]
    fn topology_fingerprint_is_order_independent() {
        let a = vec![
            mon(0, Some("uuid-a"), "DP-1", Rotation::None),
            mon(1, Some("uuid-b"), "DP-2", Rotation::Right),
        ];
        let b = vec![
            mon(1, Some("uuid-b"), "DP-2", Rotation::Right),
            mon(0, Some("uuid-a"), "DP-1", Rotation::None),
        ];
        assert_eq!(
            TopologyFingerprint::from_monitors(&a),
            TopologyFingerprint::from_monitors(&b)
        );
    }

    #[test]
    fn topology_fingerprint_differs_for_rotation_change() {
        let a = vec![mon(0, Some("x"), "DP-1", Rotation::None)];
        let b = vec![mon(0, Some("x"), "DP-1", Rotation::Right)];
        assert_ne!(
            TopologyFingerprint::from_monitors(&a),
            TopologyFingerprint::from_monitors(&b)
        );
    }

    #[test]
    fn topology_fingerprint_falls_back_to_name_when_stable_id_absent() {
        let with_id = vec![mon(0, Some("DP-1"), "DP-1", Rotation::None)];
        let without_id = vec![mon(0, None, "DP-1", Rotation::None)];
        assert_eq!(
            TopologyFingerprint::from_monitors(&with_id),
            TopologyFingerprint::from_monitors(&without_id)
        );
    }

    #[test]
    fn schedule_round_trips_through_toml() {
        let s = Schedule {
            display_name: Some("evening".to_owned()),
            profile: "dark".to_owned(),
            trigger: Trigger::Daily {
                hour: 18,
                minute: 0,
            },
            enabled: true,
        };
        let toml_text = toml::to_string(&s).unwrap();
        let back: Schedule = toml::from_str(&toml_text).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn validate_trigger_rejects_invalid_daily() {
        let bad = Trigger::Daily {
            hour: 25,
            minute: 0,
        };
        assert!(matches!(
            validate_trigger(&bad),
            Err(ScheduleError::InvalidDaily { .. })
        ));
    }

    #[test]
    fn validate_trigger_rejects_unparsable_cron() {
        let bad = Trigger::Cron {
            expr: "this is not cron".to_owned(),
        };
        assert!(matches!(
            validate_trigger(&bad),
            Err(ScheduleError::InvalidCron { .. })
        ));
    }
}
