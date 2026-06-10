//! Hyprland display detector backed by `hyprctl monitors -j`.

use std::env;
use std::time::Duration;

use serde::Deserialize;

use super::{Availability, DetectError, DisplayDetector, Monitor, MonitorId, Rotation};

const TOOL: &str = "hyprctl";
const CMD: &str = "hyprctl monitors -j";
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct HyprctlDetector;

impl DisplayDetector for HyprctlDetector {
    fn name(&self) -> &str {
        TOOL
    }

    fn availability(&self) -> Availability {
        if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            return Availability::WrongEnvironment {
                reason: "$HYPRLAND_INSTANCE_SIGNATURE not set",
            };
        }
        if super::which(TOOL).is_none() {
            return Availability::ToolMissing { tool: TOOL };
        }
        Availability::Available
    }

    fn detect(&self) -> Result<Vec<Monitor>, DetectError> {
        let stdout =
            super::run_subprocess(TOOL, &["monitors".as_ref(), "-j".as_ref()], TIMEOUT, CMD)?;
        let monitors = parse_hyprctl_json(&stdout, CMD)?;
        if monitors.is_empty() {
            return Err(DetectError::EmptyResult);
        }
        Ok(monitors)
    }
}

#[derive(Debug, Deserialize)]
struct RawMonitor {
    name: String,
    width: u32,
    height: u32,
    #[serde(rename = "refreshRate", default)]
    refresh_rate: Option<f64>,
    x: i32,
    y: i32,
    #[serde(default)]
    scale: Option<f64>,
    #[serde(default)]
    transform: u8,
    #[serde(default)]
    serial: Option<String>,
    #[serde(default)]
    disabled: bool,
}

/// Parse `hyprctl monitors -j` stdout into [`Monitor`]s. Flipped transforms
/// (4–7) are rejected — v1 doesn't model mirrored layouts.
pub(crate) fn parse_hyprctl_json(
    stdout: &str,
    cmd_name: &str,
) -> Result<Vec<Monitor>, DetectError> {
    let raw: Vec<RawMonitor> = serde_json::from_str(stdout).map_err(|e| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("invalid JSON: {e}"),
    })?;

    let mut monitors = Vec::new();
    let mut next_id: u32 = 0;
    for output in raw {
        if output.disabled {
            continue;
        }
        let rotation = parse_transform(output.transform, cmd_name, &output.name)?;
        let stable_id = output
            .serial
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let refresh_hz = output.refresh_rate.and_then(|hz| {
            #[allow(clippy::cast_possible_truncation)] // reason: refresh rates fit easily in f32
            let cast = hz as f32;
            cast.is_finite().then_some(cast)
        });
        monitors.push(Monitor {
            id: MonitorId(next_id),
            name: output.name,
            stable_id,
            position: (output.x, output.y),
            resolution: (output.width, output.height),
            physical_size_mm: None,
            scale: output.scale.unwrap_or(1.0),
            rotation,
            refresh_hz,
            ppi: None,
        });
        next_id = next_id.saturating_add(1);
    }
    Ok(monitors)
}

fn parse_transform(value: u8, cmd_name: &str, output_name: &str) -> Result<Rotation, DetectError> {
    match value {
        0 => Ok(Rotation::None),
        1 => Ok(Rotation::Right),
        2 => Ok(Rotation::Inverted),
        3 => Ok(Rotation::Left),
        other => Err(DetectError::Parse {
            cmd: cmd_name.to_owned(),
            message: format!(
                "output '{output_name}': transform {other} (flipped variant) is not supported"
            ),
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on fixture parse errors
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/display/hyprctl-2-monitors.json");

    #[test]
    fn two_monitor_fixture_parses_to_expected_layout() {
        let monitors = parse_hyprctl_json(FIXTURE, CMD).unwrap();
        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0].name, "DP-1");
        assert_eq!(monitors[0].resolution, (2560, 1440));
        assert_eq!(monitors[0].position, (0, 0));
        assert_eq!(monitors[1].name, "HDMI-A-1");
        assert_eq!(monitors[1].resolution, (1920, 1080));
        assert_eq!(monitors[1].position, (2560, 0));
    }

    #[test]
    fn fixture_uses_serial_field_verbatim_as_stable_id() {
        let monitors = parse_hyprctl_json(FIXTURE, CMD).unwrap();
        assert_eq!(monitors[0].stable_id.as_deref(), Some("ABC1234567"));
        assert_eq!(monitors[1].stable_id.as_deref(), Some("XYZ9876543"));
    }

    #[test]
    fn missing_serial_yields_no_stable_id() {
        let json = r#"[{"name":"DP-1","width":1920,"height":1080,"refreshRate":60.0,"x":0,"y":0,"scale":1.0,"transform":0,"focused":true}]"#;
        let monitors = parse_hyprctl_json(json, CMD).unwrap();
        assert_eq!(monitors[0].stable_id, None);
    }

    #[test]
    fn disabled_monitor_is_excluded_from_result() {
        let json = r#"[
            {"name":"DP-1","width":1920,"height":1080,"refreshRate":60.0,"x":0,"y":0,"scale":1.0,"transform":0,"disabled":true,"focused":false},
            {"name":"DP-2","width":2560,"height":1440,"refreshRate":60.0,"x":0,"y":0,"scale":1.0,"transform":0,"focused":true}
        ]"#;
        let monitors = parse_hyprctl_json(json, CMD).unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "DP-2");
    }

    #[test]
    fn flipped_transform_returns_parse_error() {
        let json = r#"[{"name":"DP-1","width":1920,"height":1080,"refreshRate":60.0,"x":0,"y":0,"scale":1.0,"transform":4,"focused":true}]"#;
        let err = parse_hyprctl_json(json, CMD).unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));
    }

    #[test]
    fn rotation_codes_map_to_enum_variants() {
        for (code, expected) in [
            (0u8, Rotation::None),
            (1, Rotation::Right),
            (2, Rotation::Inverted),
            (3, Rotation::Left),
        ] {
            let json = format!(
                r#"[{{"name":"DP-1","width":1920,"height":1080,"refreshRate":60.0,"x":0,"y":0,"scale":1.0,"transform":{code},"focused":true}}]"#
            );
            let monitors = parse_hyprctl_json(&json, CMD).unwrap();
            assert_eq!(monitors[0].rotation, expected);
        }
    }
}
