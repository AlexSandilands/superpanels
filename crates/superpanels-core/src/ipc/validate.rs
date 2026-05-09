//! Bounded validators for hostile-webview IPC inputs (`SPEC §17`).
//!
//! Daemon and in-process handlers share these helpers so neither path drifts
//! from the contract. Bounds target *human plausibility, not perf*: hitting a
//! cap means the input is malicious or malformed.

use serde_json::Value;
use thiserror::Error;

use crate::config::MonitorIdentifier;

/// Maximum monitor count in a saved config.
pub const MAX_MONITORS: usize = 64;
/// Maximum saved profile count.
pub const MAX_PROFILES: usize = 256;
/// Maximum profile-name length, post-trim, in characters.
pub const MAX_PROFILE_NAME_CHARS: usize = 64;
/// Maximum slideshow image-pool size per profile.
pub const MAX_SLIDESHOW_IMAGES: usize = 10_000;
/// Maximum monitor `stable_id` / `name` length, in characters.
pub const MAX_MONITOR_ID_CHARS: usize = 256;
/// Maximum library tag length, post-trim, in characters.
pub const MAX_TAG_CHARS: usize = 64;
/// Maximum monitor physical edge length in millimetres (10 m — well past any
/// real panel).
pub const MAX_PHYSICAL_MM: f64 = 10_000.0;
/// Maximum `|offset_px|` per axis for `preview_crop`.
pub const MAX_PREVIEW_OFFSET_PX: i32 = 1_000_000;
/// Maximum `image_size_px` per axis for `preview_crop`.
pub const MAX_PREVIEW_SIZE_PX: u32 = 1_000_000;
/// Maximum `|bezel_*_mm|` accepted for `preview_crop`. 1 km of bezel is well
/// past any real panel; the bound exists so the f64→f32 narrowing can't
/// produce ±inf or NaN.
pub const MAX_BEZEL_MM: f64 = 1_000_000.0;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{0}")]
pub struct ValidationError(pub String);

impl ValidationError {
    fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

/// Validate `physical_mm` for `set_monitor_physical_size` and config monitors.
pub fn validate_physical_mm(raw: [f64; 2]) -> Result<[f64; 2], ValidationError> {
    for (i, v) in raw.iter().enumerate() {
        if !v.is_finite() {
            return Err(ValidationError::new(format!(
                "physical_mm[{i}] must be finite"
            )));
        }
        if !(*v > 0.0 && *v <= MAX_PHYSICAL_MM) {
            return Err(ValidationError::new(format!(
                "physical_mm[{i}] = {v} must be in (0, {MAX_PHYSICAL_MM}] mm"
            )));
        }
    }
    Ok(raw)
}

/// Validate a monitor identifier string (`stable_id` or `name`).
pub fn validate_monitor_id_string<'a>(s: &'a str, field: &str) -> Result<&'a str, ValidationError> {
    if s.is_empty() {
        return Err(ValidationError::new(format!("{field} must be non-empty")));
    }
    if s.chars().count() > MAX_MONITOR_ID_CHARS {
        return Err(ValidationError::new(format!(
            "{field} exceeds {MAX_MONITOR_ID_CHARS} chars"
        )));
    }
    if s.chars().any(char::is_control) {
        return Err(ValidationError::new(format!(
            "{field} contains control characters"
        )));
    }
    Ok(s)
}

/// Validate and normalise a library tag (post-trim, lowercased downstream).
pub fn validate_tag(raw: &str) -> Result<String, ValidationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new("tag must be non-empty"));
    }
    if trimmed.chars().count() > MAX_TAG_CHARS {
        return Err(ValidationError::new(format!(
            "tag exceeds {MAX_TAG_CHARS} chars"
        )));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ValidationError::new("tag contains control characters"));
    }
    Ok(trimmed.to_owned())
}

/// Validate a profile name (post-trim).
pub fn validate_profile_name(raw: &str) -> Result<&str, ValidationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new("profile name must be non-empty"));
    }
    if trimmed.chars().count() > MAX_PROFILE_NAME_CHARS {
        return Err(ValidationError::new(format!(
            "profile name exceeds {MAX_PROFILE_NAME_CHARS} chars"
        )));
    }
    Ok(trimmed)
}

/// Validate `offset_px` for `preview_crop`.
pub fn validate_preview_offset(raw: [i32; 2]) -> Result<[i32; 2], ValidationError> {
    for (i, &v) in raw.iter().enumerate() {
        if !(-MAX_PREVIEW_OFFSET_PX..=MAX_PREVIEW_OFFSET_PX).contains(&v) {
            return Err(ValidationError::new(format!(
                "offset_px[{i}] = {v} exceeds ±{MAX_PREVIEW_OFFSET_PX}"
            )));
        }
    }
    Ok(raw)
}

/// Validate `image_size_px` for `preview_crop`.
pub fn validate_preview_image_size(raw: [u32; 2]) -> Result<[u32; 2], ValidationError> {
    for (i, &v) in raw.iter().enumerate() {
        if v > MAX_PREVIEW_SIZE_PX {
            return Err(ValidationError::new(format!(
                "image_size_px[{i}] = {v} exceeds {MAX_PREVIEW_SIZE_PX}"
            )));
        }
    }
    Ok(raw)
}

/// Pull `params.stable_id` (preferred) or `params.name` from a JSON params
/// object and validate the chosen identifier into a [`MonitorIdentifier`].
/// `stable_id` wins when both are set and non-empty.
pub fn parse_monitor_identifier(params: &Value) -> Result<MonitorIdentifier, ValidationError> {
    if let Some(id) = params.get("stable_id").and_then(Value::as_str) {
        if !id.is_empty() {
            return validate_monitor_id_string(id, "stable_id")
                .map(|s| MonitorIdentifier::StableId(s.to_owned()));
        }
    }
    if let Some(name) = params.get("name").and_then(Value::as_str) {
        if !name.is_empty() {
            return validate_monitor_id_string(name, "name")
                .map(|s| MonitorIdentifier::Name(s.to_owned()));
        }
    }
    Err(ValidationError::new(
        "params.stable_id or params.name (non-empty string) required",
    ))
}

/// Pull `params.physical_mm` and validate it as a `[f64; 2]`.
pub fn parse_physical_mm(params: &Value) -> Result<[f64; 2], ValidationError> {
    let val = params
        .get("physical_mm")
        .ok_or_else(|| ValidationError::new("params.physical_mm required"))?;
    let raw: [f64; 2] = serde_json::from_value(val.clone())
        .map_err(|e| ValidationError::new(format!("physical_mm must be [number, number]: {e}")))?;
    validate_physical_mm(raw)
}

/// Bound-check a bezel measurement and narrow `f64 → f32`.
///
/// `BezelConfig::{horizontal_mm,vertical_mm}` are `f32`, but the wire format
/// is `f64`. The bounds check rejects ±inf, NaN, and absurd magnitudes so the
/// `as f32` cast can't silently produce ±inf.
pub fn validate_bezel_mm(v: f64) -> Result<f32, ValidationError> {
    if !v.is_finite() {
        return Err(ValidationError::new("bezel_*_mm must be finite"));
    }
    if !(-MAX_BEZEL_MM..=MAX_BEZEL_MM).contains(&v) {
        return Err(ValidationError::new(format!(
            "bezel_*_mm = {v} exceeds ±{MAX_BEZEL_MM}"
        )));
    }
    // reason: bound-checked above so the narrowing cast is well-defined.
    #[allow(clippy::cast_possible_truncation)]
    Ok(v as f32)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests panic loudly on harness bugs
#[allow(clippy::panic)] // reason: same — explicit panic on unexpected enum branch
mod tests {
    use super::*;

    #[test]
    fn physical_mm_accepts_typical_panel_dimensions() {
        let out = validate_physical_mm([597.0, 336.0]).unwrap();
        assert!((out[0] - 597.0).abs() < f64::EPSILON);
        assert!((out[1] - 336.0).abs() < f64::EPSILON);
    }

    #[test]
    fn physical_mm_rejects_zero_or_negative() {
        assert!(validate_physical_mm([0.0, 100.0]).is_err());
        assert!(validate_physical_mm([100.0, -1.0]).is_err());
    }

    #[test]
    fn physical_mm_rejects_non_finite() {
        assert!(validate_physical_mm([f64::NAN, 100.0]).is_err());
        assert!(validate_physical_mm([100.0, f64::INFINITY]).is_err());
    }

    #[test]
    fn physical_mm_rejects_above_cap() {
        assert!(validate_physical_mm([MAX_PHYSICAL_MM + 0.1, 100.0]).is_err());
        assert!(validate_physical_mm([100.0, 1e30]).is_err());
    }

    #[test]
    fn physical_mm_accepts_exact_cap() {
        let out = validate_physical_mm([MAX_PHYSICAL_MM, MAX_PHYSICAL_MM]).unwrap();
        assert!((out[0] - MAX_PHYSICAL_MM).abs() < f64::EPSILON);
        assert!((out[1] - MAX_PHYSICAL_MM).abs() < f64::EPSILON);
    }

    #[test]
    fn monitor_id_accepts_typical_kde_uuid() {
        let uuid = "abcdef01-2345-6789-abcd-ef0123456789";
        assert_eq!(validate_monitor_id_string(uuid, "stable_id").unwrap(), uuid);
    }

    #[test]
    fn monitor_id_rejects_empty() {
        assert!(validate_monitor_id_string("", "stable_id").is_err());
    }

    #[test]
    fn monitor_id_rejects_oversize() {
        let big = "x".repeat(MAX_MONITOR_ID_CHARS + 1);
        assert!(validate_monitor_id_string(&big, "stable_id").is_err());
    }

    #[test]
    fn monitor_id_rejects_control_chars() {
        assert!(validate_monitor_id_string("DP-1\nname=injected", "name").is_err());
        assert!(validate_monitor_id_string("DP-1\0", "name").is_err());
    }

    #[test]
    fn tag_trims_and_normalises() {
        assert_eq!(validate_tag("  blue  ").unwrap(), "blue");
    }

    #[test]
    fn tag_rejects_empty_or_whitespace() {
        assert!(validate_tag("").is_err());
        assert!(validate_tag("   ").is_err());
    }

    #[test]
    fn tag_rejects_oversize_after_trim() {
        let big = "a".repeat(MAX_TAG_CHARS + 1);
        assert!(validate_tag(&big).is_err());
    }

    #[test]
    fn tag_rejects_control_chars() {
        assert!(validate_tag("blue\nred").is_err());
    }

    #[test]
    fn profile_name_trims() {
        assert_eq!(validate_profile_name(" home ").unwrap(), "home");
    }

    #[test]
    fn profile_name_rejects_empty_or_oversize() {
        assert!(validate_profile_name("").is_err());
        let big = "a".repeat(MAX_PROFILE_NAME_CHARS + 1);
        assert!(validate_profile_name(&big).is_err());
    }

    #[test]
    fn preview_offset_accepts_zero() {
        assert_eq!(validate_preview_offset([0, 0]).unwrap(), [0, 0]);
    }

    #[test]
    fn preview_offset_rejects_above_cap() {
        assert!(validate_preview_offset([MAX_PREVIEW_OFFSET_PX + 1, 0]).is_err());
        assert!(validate_preview_offset([0, -MAX_PREVIEW_OFFSET_PX - 1]).is_err());
    }

    #[test]
    fn preview_image_size_accepts_4k() {
        assert_eq!(
            validate_preview_image_size([3840, 2160]).unwrap(),
            [3840, 2160]
        );
    }

    #[test]
    fn preview_image_size_rejects_above_cap() {
        assert!(validate_preview_image_size([MAX_PREVIEW_SIZE_PX + 1, 0]).is_err());
    }

    #[test]
    fn bezel_mm_accepts_typical_panel_bezel() {
        // 5 mm is a fairly typical thin-bezel monitor; round-trips through
        // f32 cleanly so the comparison is straightforward.
        assert!((validate_bezel_mm(5.0).unwrap() - 5.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn bezel_mm_accepts_zero_and_signed_values() {
        // Negative bezels are nonsensical physically but the geometry code
        // accepts them; the validator's job is just bounds + finiteness.
        assert!(validate_bezel_mm(0.0).unwrap().abs() < f32::EPSILON);
        assert!((validate_bezel_mm(-3.0).unwrap() + 3.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn bezel_mm_rejects_non_finite() {
        assert!(validate_bezel_mm(f64::NAN).is_err());
        assert!(validate_bezel_mm(f64::INFINITY).is_err());
        assert!(validate_bezel_mm(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn bezel_mm_rejects_above_cap() {
        assert!(validate_bezel_mm(MAX_BEZEL_MM + 1.0).is_err());
        assert!(validate_bezel_mm(-MAX_BEZEL_MM - 1.0).is_err());
        assert!(validate_bezel_mm(1e30).is_err());
    }

    #[test]
    fn parse_monitor_identifier_prefers_stable_id_over_name() {
        let params = serde_json::json!({"stable_id": "uuid-x", "name": "DP-1"});
        match parse_monitor_identifier(&params).unwrap() {
            MonitorIdentifier::StableId(s) => assert_eq!(s, "uuid-x"),
            MonitorIdentifier::Name(_) => panic!("expected StableId branch"),
        }
    }

    #[test]
    fn parse_monitor_identifier_falls_back_to_name_when_stable_id_missing_or_empty() {
        let params = serde_json::json!({"stable_id": "", "name": "DP-1"});
        match parse_monitor_identifier(&params).unwrap() {
            MonitorIdentifier::Name(s) => assert_eq!(s, "DP-1"),
            MonitorIdentifier::StableId(_) => panic!("expected Name branch"),
        }
        let params = serde_json::json!({"name": "DP-1"});
        assert!(matches!(
            parse_monitor_identifier(&params).unwrap(),
            MonitorIdentifier::Name(_)
        ));
    }

    #[test]
    fn parse_monitor_identifier_rejects_when_neither_present() {
        assert!(parse_monitor_identifier(&serde_json::json!({})).is_err());
        assert!(
            parse_monitor_identifier(&serde_json::json!({"stable_id": "", "name": ""})).is_err()
        );
    }

    #[test]
    fn parse_monitor_identifier_propagates_validate_failures() {
        // Control characters get rejected by `validate_monitor_id_string`; the
        // wrapper must propagate, not silently fall through to the `name` branch.
        let params = serde_json::json!({"stable_id": "DP-1\nname=injected", "name": "DP-1"});
        assert!(parse_monitor_identifier(&params).is_err());
    }

    #[test]
    fn parse_physical_mm_round_trips_typical_values() {
        let params = serde_json::json!({"physical_mm": [597.0, 336.0]});
        let mm = parse_physical_mm(&params).unwrap();
        assert!((mm[0] - 597.0).abs() < f64::EPSILON);
        assert!((mm[1] - 336.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_physical_mm_rejects_missing_or_malformed() {
        assert!(parse_physical_mm(&serde_json::json!({})).is_err());
        assert!(parse_physical_mm(&serde_json::json!({"physical_mm": "abc"})).is_err());
        assert!(parse_physical_mm(&serde_json::json!({"physical_mm": [597.0]})).is_err());
        // Validation cap still applies after JSON extraction.
        assert!(parse_physical_mm(&serde_json::json!({"physical_mm": [0.0, 100.0]})).is_err());
    }
}
