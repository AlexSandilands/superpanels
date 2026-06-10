//! KDE Plasma display detector backed by `kscreen-doctor -o`.
//!
//! Physical mm are not extracted (kscreen-doctor doesn't expose them); they
//! come from `[[monitor]]` config.

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use super::{Availability, DetectError, DisplayDetector, Monitor, MonitorId, Rotation};

const TOOL: &str = "kscreen-doctor";
const CMD: &str = "kscreen-doctor -o";
const TIMEOUT: Duration = Duration::from_secs(5);

type Position = (i32, i32);
type Resolution = (u32, u32);
type Geometry = (Position, Resolution);

#[derive(Debug)]
pub struct KscreenDoctorDetector;

impl DisplayDetector for KscreenDoctorDetector {
    fn name(&self) -> &str {
        TOOL
    }

    fn availability(&self) -> Availability {
        let in_kde = env::var_os("KDE_FULL_SESSION").is_some()
            || env::var("XDG_CURRENT_DESKTOP")
                .is_ok_and(|d| d.split(':').any(|s| s.eq_ignore_ascii_case("KDE")));
        if !in_kde {
            return Availability::WrongEnvironment {
                reason: "$KDE_FULL_SESSION not set and $XDG_CURRENT_DESKTOP does not contain KDE",
            };
        }
        if which(TOOL).is_none() {
            return Availability::ToolMissing { tool: TOOL };
        }
        Availability::Available
    }

    fn detect(&self) -> Result<Vec<Monitor>, DetectError> {
        let output = run_with_timeout()?;
        let monitors = parse_kscreen_output(&output, CMD)?;
        if monitors.is_empty() {
            return Err(DetectError::EmptyResult);
        }
        Ok(monitors)
    }
}

fn run_with_timeout() -> Result<String, DetectError> {
    let mut child = Command::new(TOOL)
        .arg("-o")
        .env("NO_COLOR", "1")
        .env("LC_ALL", "C")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| DetectError::Subprocess {
            cmd: CMD.to_owned(),
            stderr: e.to_string(),
        })?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| DetectError::Subprocess {
                        cmd: CMD.to_owned(),
                        stderr: e.to_string(),
                    })?;
                if !status.success() {
                    return Err(DetectError::Subprocess {
                        cmd: CMD.to_owned(),
                        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                    });
                }
                return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(None) => {
                if start.elapsed() >= TIMEOUT {
                    let _ = child.kill();
                    return Err(DetectError::Timeout {
                        cmd: CMD.to_owned(),
                        seconds: TIMEOUT.as_secs(),
                    });
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                return Err(DetectError::Subprocess {
                    cmd: CMD.to_owned(),
                    stderr: e.to_string(),
                });
            }
        }
    }
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Parse `kscreen-doctor -o` stdout into [`Monitor`]s.
///
/// ANSI escapes are stripped defensively in case `NO_COLOR=1` is ignored.
pub(crate) fn parse_kscreen_output(
    output: &str,
    cmd_name: &str,
) -> Result<Vec<Monitor>, DetectError> {
    let cleaned = strip_ansi(output);
    let mut monitors = Vec::new();
    let mut current: Option<RawOutput> = None;
    let mut next_id: u32 = 0;

    for (lineno, raw_line) in cleaned.lines().enumerate() {
        let line = raw_line.trim_start();
        if let Some(rest) = line.strip_prefix("Output:") {
            if let Some(prev) = current.take()
                && let Some(monitor) = finalize(prev, &mut next_id, cmd_name, lineno)?
            {
                monitors.push(monitor);
            }
            current = Some(parse_output_header(rest.trim(), cmd_name, lineno + 1)?);
        } else if let Some(state) = current.as_mut() {
            absorb_field(state, line, cmd_name, lineno + 1)?;
        }
    }

    if let Some(prev) = current.take()
        && let Some(monitor) = finalize(prev, &mut next_id, cmd_name, 0)?
    {
        monitors.push(monitor);
    }

    Ok(monitors)
}

#[derive(Debug, Default)]
struct RawOutput {
    name: String,
    stable_id: Option<String>,
    enabled: bool,
    connected: bool,
    geometry: Option<Geometry>,
    scale: Option<f64>,
    rotation: Option<Rotation>,
    refresh_hz: Option<f32>,
}

fn parse_output_header(
    rest: &str,
    cmd_name: &str,
    lineno: usize,
) -> Result<RawOutput, DetectError> {
    // "<index> <NAME> [<UUID>]"
    let mut parts = rest.split_whitespace();
    parts.next().ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: missing output index"),
    })?;
    let name = parts.next().ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: missing output name"),
    })?;
    let stable_id = parts.next().map(str::to_owned);
    Ok(RawOutput {
        name: name.to_owned(),
        stable_id,
        ..RawOutput::default()
    })
}

fn absorb_field(
    state: &mut RawOutput,
    line: &str,
    cmd_name: &str,
    lineno: usize,
) -> Result<(), DetectError> {
    let trimmed = line.trim();
    match trimmed {
        "enabled" => state.enabled = true,
        "disabled" => state.enabled = false,
        "connected" => state.connected = true,
        "disconnected" => state.connected = false,
        _ => {}
    }

    if let Some(rest) = trimmed.strip_prefix("Geometry:") {
        state.geometry = Some(parse_geometry(rest.trim(), cmd_name, lineno)?);
    } else if let Some(rest) = trimmed.strip_prefix("Scale:") {
        let s: f64 = rest.trim().parse().map_err(|e| DetectError::Parse {
            cmd: cmd_name.to_owned(),
            message: format!("line {lineno}: invalid scale '{}': {e}", rest.trim()),
        })?;
        state.scale = Some(s);
    } else if let Some(rest) = trimmed.strip_prefix("Rotation:") {
        state.rotation = Some(parse_rotation(rest.trim(), cmd_name, lineno)?);
    } else if let Some(rest) = trimmed.strip_prefix("Modes:") {
        state.refresh_hz = parse_active_refresh(rest);
    }
    Ok(())
}

fn parse_geometry(rest: &str, cmd_name: &str, lineno: usize) -> Result<Geometry, DetectError> {
    // "X,Y WxH"
    let mut parts = rest.split_whitespace();
    let pos = parts.next().ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: Geometry missing position"),
    })?;
    let size = parts.next().ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: Geometry missing size"),
    })?;
    let (xs, ys) = pos.split_once(',').ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: Geometry position '{pos}' is not 'X,Y'"),
    })?;
    let x: i32 = xs.parse().map_err(|e| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: invalid Geometry x '{xs}': {e}"),
    })?;
    let y: i32 = ys.parse().map_err(|e| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: invalid Geometry y '{ys}': {e}"),
    })?;
    let (ws, hs) = size.split_once('x').ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: Geometry size '{size}' is not 'WxH'"),
    })?;
    let w: u32 = ws.parse().map_err(|e| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: invalid Geometry width '{ws}': {e}"),
    })?;
    let h: u32 = hs.parse().map_err(|e| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: invalid Geometry height '{hs}': {e}"),
    })?;
    Ok(((x, y), (w, h)))
}

fn parse_rotation(rest: &str, cmd_name: &str, lineno: usize) -> Result<Rotation, DetectError> {
    match rest {
        "1" | "none" | "normal" => Ok(Rotation::None),
        "2" | "right" => Ok(Rotation::Right),
        "4" | "inverted" => Ok(Rotation::Inverted),
        "8" | "left" => Ok(Rotation::Left),
        other => Err(DetectError::Parse {
            cmd: cmd_name.to_owned(),
            message: format!("line {lineno}: unknown Rotation value '{other}'"),
        }),
    }
}

fn parse_active_refresh(modes: &str) -> Option<f32> {
    // Active entry: " 2:2560x1440@155.00*" — trailing '*' marks it.
    for tok in modes.split_whitespace() {
        let unstar = tok.trim_end_matches('*');
        if unstar.len() == tok.len() {
            continue;
        }
        let after_at = unstar.rsplit_once('@')?.1;
        if let Ok(hz) = after_at.parse::<f32>() {
            return Some(hz);
        }
    }
    None
}

fn finalize(
    raw: RawOutput,
    next_id: &mut u32,
    cmd_name: &str,
    lineno: usize,
) -> Result<Option<Monitor>, DetectError> {
    if !(raw.enabled && raw.connected) {
        return Ok(None);
    }
    let (position, geom_resolution) = raw.geometry.ok_or_else(|| DetectError::Parse {
        cmd: cmd_name.to_owned(),
        message: format!("line {lineno}: output '{}' missing Geometry", raw.name),
    })?;
    let rotation = raw.rotation.unwrap_or(Rotation::None);
    // `kscreen-doctor` reports `Geometry` in compositor (post-rotation) space.
    // requires `Monitor.resolution` in native panel orientation; the
    // layout module re-applies the rotation. Swap back when rotated so the
    // invariant matches wlr-randr (which already reports native mode w/h).
    let resolution = match rotation {
        Rotation::Left | Rotation::Right => (geom_resolution.1, geom_resolution.0),
        Rotation::None | Rotation::Inverted => geom_resolution,
    };
    let monitor = Monitor {
        id: MonitorId(*next_id),
        name: raw.name,
        stable_id: raw.stable_id,
        position,
        resolution,
        physical_size_mm: None,
        scale: raw.scale.unwrap_or(1.0),
        rotation,
        refresh_hz: raw.refresh_hz,
        ppi: None,
    };
    *next_id += 1;
    Ok(Some(monitor))
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly when fixture parsing breaks
mod tests {
    use super::*;

    const THREE: &str = include_str!("../../tests/fixtures/display/kscreen-3-monitors.txt");
    const ONE: &str = include_str!("../../tests/fixtures/display/kscreen-1-monitor.txt");

    #[test]
    fn three_monitor_fixture_parses_to_expected_layout() {
        let monitors = parse_kscreen_output(THREE, CMD).unwrap();
        insta::assert_debug_snapshot!(monitors);
    }

    #[test]
    fn single_monitor_fixture_parses_to_expected_layout() {
        let monitors = parse_kscreen_output(ONE, CMD).unwrap();
        insta::assert_debug_snapshot!(monitors);
    }

    #[test]
    fn ansi_escape_sequences_are_stripped_before_parse() {
        let with_ansi = "\x1b[01;32mOutput: \x1b[0;0m1 DP-1 abc-uuid\n\
                         \tenabled\n\
                         \tconnected\n\
                         \tpriority 1\n\
                         \tGeometry: 0,0 1920x1080\n\
                         \tScale: 1\n\
                         \tRotation: 1\n";
        let monitors = parse_kscreen_output(with_ansi, CMD).unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "DP-1");
        assert_eq!(monitors[0].resolution, (1920, 1080));
    }

    #[test]
    fn disabled_outputs_are_excluded_from_result() {
        let input = "Output: 1 DP-1 uuid-a\n\
                     \tdisabled\n\
                     \tconnected\n\
                     \tGeometry: 0,0 1920x1080\n\
                     \tRotation: 1\n\
                     Output: 2 DP-2 uuid-b\n\
                     \tenabled\n\
                     \tconnected\n\
                     \tpriority 1\n\
                     \tGeometry: 0,0 2560x1440\n\
                     \tScale: 1\n\
                     \tRotation: 1\n";
        let monitors = parse_kscreen_output(input, CMD).unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "DP-2");
    }

    #[test]
    fn rotation_codes_map_to_enum_variants() {
        for (code, expected) in [
            ("1", Rotation::None),
            ("2", Rotation::Right),
            ("4", Rotation::Inverted),
            ("8", Rotation::Left),
        ] {
            let input = format!(
                "Output: 1 DP-1 u\n\tenabled\n\tconnected\n\tpriority 1\n\
                 \tGeometry: 0,0 1920x1080\n\tScale: 1\n\tRotation: {code}\n"
            );
            let monitors = parse_kscreen_output(&input, CMD).unwrap();
            assert_eq!(monitors[0].rotation, expected);
        }
    }

    #[test]
    fn missing_geometry_returns_parse_error() {
        let input = "Output: 1 DP-1 u\n\tenabled\n\tconnected\n\tpriority 1\n\tRotation: 1\n";
        let err = parse_kscreen_output(input, CMD).unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));
    }
}
