//! wlroots display detector backed by `wlr-randr --json` (`SPEC.md` §6.4).

use std::env;
use std::time::Duration;

use serde::Deserialize;

use super::stable_id::hash_edid_triple;
use super::{Availability, DetectError, DisplayDetector, Monitor, MonitorId, Rotation};

const TOOL: &str = "wlr-randr";
const CMD: &str = "wlr-randr --json";
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct WlrRandrDetector;

impl DisplayDetector for WlrRandrDetector {
    fn name(&self) -> &str {
        TOOL
    }

    fn availability(&self) -> Availability {
        if env::var_os("WAYLAND_DISPLAY").is_none() {
            return Availability::WrongEnvironment {
                reason: "$WAYLAND_DISPLAY not set",
            };
        }
        if env::var_os("SWAYSOCK").is_some() {
            return Availability::WrongEnvironment {
                reason: "$SWAYSOCK is set; the dedicated Sway detector handles this session",
            };
        }
        if super::which(TOOL).is_none() {
            return Availability::ToolMissing { tool: TOOL };
        }
        Availability::Available
    }

    fn detect(&self) -> Result<Vec<Monitor>, DetectError> {
        let stdout = super::run_subprocess(TOOL, &["--json".as_ref()], TIMEOUT, CMD)?;
        let monitors = parse_wlr_randr_json(&stdout, CMD)?;
        if monitors.is_empty() {
            return Err(DetectError::EmptyResult);
        }
        Ok(monitors)
    }
}

#[derive(Debug, Deserialize)]
struct RawOutput {
    name: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    make: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    serial: Option<String>,
    #[serde(default)]
    position: Option<RawPosition>,
    #[serde(default)]
    transform: Option<String>,
    #[serde(default)]
    scale: Option<f64>,
    #[serde(default)]
    modes: Vec<RawMode>,
    #[serde(default, rename = "physical_size")]
    physical_size: Option<RawSize>,
}

#[derive(Debug, Deserialize)]
struct RawPosition {
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize)]
struct RawSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize)]
struct RawMode {
    width: u32,
    height: u32,
    #[serde(default)]
    refresh: Option<f64>,
    #[serde(default)]
    current: bool,
}

/// Parse `wlr-randr --json` stdout into [`Monitor`]s. Missing position defaults
/// to `(0, 0)` — the compositor returns `null` for unarranged outputs.
pub(crate) fn parse_wlr_randr_json(
    stdout: &str,
    cmd_name: &str,
) -> Result<Vec<Monitor>, DetectError> {
    let raw: Vec<RawOutput> = serde_json::from_str(stdout).map_err(|e| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("invalid JSON: {e}"),
    })?;

    let mut monitors = Vec::new();
    let mut next_id: u32 = 0;

    for output in raw {
        if !output.enabled {
            continue;
        }
        let active = output
            .modes
            .iter()
            .find(|m| m.current)
            .ok_or_else(|| DetectError::Parse {
                cmd: cmd_name.to_owned(),
                message: format!("output '{}' has no active mode", output.name),
            })?;
        let resolution = (active.width, active.height);
        let refresh_hz = active.refresh.and_then(|hz| {
            #[allow(clippy::cast_possible_truncation)]
            // reason: refresh rates are well below f32 max
            let cast = hz as f32;
            cast.is_finite().then_some(cast)
        });
        let position = output
            .position
            .map_or((0, 0), |RawPosition { x, y }| (x, y));
        let rotation = parse_transform(output.transform.as_deref(), cmd_name, &output.name)?;
        let scale = output.scale.unwrap_or(1.0);
        let physical_size_mm = output
            .physical_size
            .map(|RawSize { width, height }| (width, height));
        let stable_id = hash_edid_triple(
            output.make.as_deref(),
            output.model.as_deref(),
            output.serial.as_deref(),
        );

        monitors.push(Monitor {
            id: MonitorId(next_id),
            name: output.name,
            stable_id,
            position,
            resolution,
            physical_size_mm,
            scale,
            rotation,
            refresh_hz,
            primary: false,
            ppi: None,
        });
        next_id = next_id.saturating_add(1);
    }

    Ok(monitors)
}

fn parse_transform(
    raw: Option<&str>,
    cmd_name: &str,
    output_name: &str,
) -> Result<Rotation, DetectError> {
    match raw.unwrap_or("normal") {
        "normal" | "0" => Ok(Rotation::None),
        "90" => Ok(Rotation::Right),
        "180" => Ok(Rotation::Inverted),
        "270" => Ok(Rotation::Left),
        other => Err(DetectError::Parse {
            cmd: cmd_name.to_owned(),
            message: format!("output '{output_name}': unknown transform '{other}'"),
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on fixture parse errors
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/display/wlr-randr-2-monitors.json");

    #[test]
    fn two_monitor_fixture_parses_to_expected_layout() {
        let monitors = parse_wlr_randr_json(FIXTURE, CMD).unwrap();
        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0].name, "DP-1");
        assert_eq!(monitors[0].resolution, (2560, 1440));
        assert_eq!(monitors[0].position, (0, 0));
        assert_eq!(monitors[1].name, "HDMI-A-1");
        assert_eq!(monitors[1].resolution, (1920, 1080));
        assert_eq!(monitors[1].position, (2560, 0));
    }

    #[test]
    fn fixture_populates_stable_id_from_make_model_serial() {
        let monitors = parse_wlr_randr_json(FIXTURE, CMD).unwrap();
        assert!(monitors[0].stable_id.is_some());
        assert!(monitors[1].stable_id.is_some());
        assert_ne!(monitors[0].stable_id, monitors[1].stable_id);
    }

    #[test]
    fn missing_make_model_serial_yields_no_stable_id() {
        let json = r#"[{"name":"DP-2","enabled":true,"modes":[{"width":1920,"height":1080,"refresh":60.0,"current":true}]}]"#;
        let monitors = parse_wlr_randr_json(json, CMD).unwrap();
        assert_eq!(monitors[0].stable_id, None);
    }

    #[test]
    fn disabled_outputs_are_excluded_from_result() {
        let json = r#"[
            {"name":"DP-1","enabled":false,"modes":[{"width":1920,"height":1080,"refresh":60.0,"current":true}]},
            {"name":"DP-2","enabled":true,"modes":[{"width":2560,"height":1440,"refresh":60.0,"current":true}]}
        ]"#;
        let monitors = parse_wlr_randr_json(json, CMD).unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "DP-2");
    }

    #[test]
    fn enabled_output_without_active_mode_returns_parse_error() {
        let json = r#"[{"name":"DP-1","enabled":true,"modes":[{"width":1920,"height":1080,"refresh":60.0,"current":false}]}]"#;
        let err = parse_wlr_randr_json(json, CMD).unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));
    }

    #[test]
    fn unknown_transform_returns_parse_error() {
        let json = r#"[{"name":"DP-1","enabled":true,"transform":"flipped","modes":[{"width":1920,"height":1080,"refresh":60.0,"current":true}]}]"#;
        let err = parse_wlr_randr_json(json, CMD).unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));
    }
}
