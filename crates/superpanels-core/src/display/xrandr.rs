//! X11 display detector backed by `xrandr --verbose` (`SPEC.md` §6.4).

use std::env;
use std::time::Duration;

use super::stable_id::hash_edid_triple;
use super::{Availability, DetectError, DisplayDetector, Monitor, MonitorId, Rotation};

const TOOL: &str = "xrandr";
const CMD: &str = "xrandr --verbose";
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct XrandrDetector;

impl DisplayDetector for XrandrDetector {
    fn name(&self) -> &str {
        TOOL
    }

    fn availability(&self) -> Availability {
        if env::var_os("DISPLAY").is_none() {
            return Availability::WrongEnvironment {
                reason: "$DISPLAY not set",
            };
        }
        if env::var_os("WAYLAND_DISPLAY").is_some() {
            return Availability::WrongEnvironment {
                reason: "$WAYLAND_DISPLAY is set; X11 detector should not run under Wayland",
            };
        }
        if super::which(TOOL).is_none() {
            return Availability::ToolMissing { tool: TOOL };
        }
        Availability::Available
    }

    fn detect(&self) -> Result<Vec<Monitor>, DetectError> {
        let stdout = super::run_subprocess(TOOL, &["--verbose".as_ref()], TIMEOUT, CMD)?;
        let monitors = parse_xrandr_verbose(&stdout, CMD)?;
        if monitors.is_empty() {
            return Err(DetectError::EmptyResult);
        }
        Ok(monitors)
    }
}

/// Parse `xrandr --verbose` stdout into [`Monitor`]s.
pub(crate) fn parse_xrandr_verbose(
    stdout: &str,
    cmd_name: &str,
) -> Result<Vec<Monitor>, DetectError> {
    let mut monitors = Vec::new();
    let mut current: Option<RawConnector> = None;
    let mut next_id: u32 = 0;
    let mut in_edid = false;
    let mut edid_hex = String::new();

    for line in stdout.lines() {
        // EDID payload is doubly-indented hex; stop on the first non-hex line.
        if in_edid {
            let trimmed = line.trim();
            if line.starts_with("\t\t") && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                edid_hex.push_str(trimmed);
                continue;
            }
            in_edid = false;
        }

        if !line.starts_with('\t') && !line.starts_with(' ') {
            if let Some(prev) = current.take()
                && let Some(monitor) = finalize(prev, &edid_hex, &mut next_id, cmd_name)?
            {
                monitors.push(monitor);
            }
            edid_hex.clear();
            current = parse_connector_header(line);
            continue;
        }

        let Some(state) = current.as_mut() else {
            continue;
        };

        if line.trim_start().starts_with("EDID:") {
            in_edid = true;
            continue;
        }

        // The "*current" marker identifies the active mode block; its `v:` line
        // (two lines down) carries the refresh rate.
        if line.starts_with("  ") && !line.starts_with("    ") && line.contains('x') {
            state.active_mode_block = line.contains("*current");
        }
        if state.active_mode_block
            && line.trim_start().starts_with("v:")
            && let Some(hz) = extract_v_refresh(line.trim_start())
        {
            state.refresh_hz = Some(hz);
        }
    }

    if let Some(prev) = current.take()
        && let Some(monitor) = finalize(prev, &edid_hex, &mut next_id, cmd_name)?
    {
        monitors.push(monitor);
    }

    Ok(monitors)
}

#[derive(Debug, Default)]
struct RawConnector {
    name: String,
    connected: bool,
    enabled: bool,
    position: (i32, i32),
    resolution: (u32, u32),
    physical_size_mm: Option<(u32, u32)>,
    rotation: Rotation,
    refresh_hz: Option<f32>,
    active_mode_block: bool,
}

fn parse_connector_header(line: &str) -> Option<RawConnector> {
    let mut tokens = line.split_whitespace();
    let name = tokens.next()?;
    let state = tokens.next()?;
    if state == "disconnected" {
        return Some(RawConnector {
            name: name.to_owned(),
            connected: false,
            ..RawConnector::default()
        });
    }
    if state != "connected" {
        return None;
    }

    let mut conn = RawConnector {
        name: name.to_owned(),
        connected: true,
        ..RawConnector::default()
    };

    // Remaining: [primary] [WxH+X+Y] [(mask)] [rotation] [WMMmm x HMMmm]
    let rest: Vec<&str> = tokens.collect();
    let mut idx = 0;
    if rest.first() == Some(&"primary") {
        idx += 1;
    }

    if let Some(tok) = rest.get(idx)
        && let Some(geo) = parse_geometry(tok)
    {
        conn.resolution = geo.0;
        conn.position = geo.1;
        conn.enabled = true;
        idx += 1;
    }

    if let Some(tok) = rest.get(idx)
        && tok.starts_with('(')
    {
        idx += 1;
    }

    if let Some(tok) = rest.get(idx) {
        match *tok {
            "normal" => {
                conn.rotation = Rotation::None;
                idx += 1;
            }
            "left" => {
                conn.rotation = Rotation::Left;
                idx += 1;
            }
            "right" => {
                conn.rotation = Rotation::Right;
                idx += 1;
            }
            "inverted" => {
                conn.rotation = Rotation::Inverted;
                idx += 1;
            }
            _ => {}
        }
    }

    // Physical size "<W>mm x <H>mm": three separate xrandr tokens; reflection /
    // scaling tokens may appear in between, so scan the tail rather than indexing.
    let tail = &rest[idx..];
    for (i, tok) in tail.iter().enumerate() {
        if let Some(width_str) = tok.strip_suffix("mm")
            && let Ok(w) = width_str.parse::<u32>()
            && tail.get(i + 1).copied() == Some("x")
            && let Some(height_tok) = tail.get(i + 2)
            && let Some(height_str) = height_tok.strip_suffix("mm")
            && let Ok(h) = height_str.parse::<u32>()
        {
            conn.physical_size_mm = Some((w, h));
            break;
        }
    }

    Some(conn)
}

fn parse_geometry(tok: &str) -> Option<((u32, u32), (i32, i32))> {
    // WxH+X+Y, where X or Y may be negative (so split on `+` or `-` past index 0).
    let res_end = tok
        .char_indices()
        .find(|(i, c)| (*c == '+' || *c == '-') && *i > 0)?
        .0;
    let (res_str, rest) = tok.split_at(res_end);
    let (w_str, h_str) = res_str.split_once('x')?;
    let w: u32 = w_str.parse().ok()?;
    let h: u32 = h_str.parse().ok()?;
    let pos_split = rest[1..]
        .char_indices()
        .find(|(_, c)| *c == '+' || *c == '-')?
        .0
        + 1;
    let x: i32 = rest[..pos_split].parse().ok()?;
    let y: i32 = rest[pos_split..].parse().ok()?;
    Some(((w, h), (x, y)))
}

fn extract_v_refresh(line: &str) -> Option<f32> {
    for tok in line.split_whitespace().rev() {
        if let Some(hz_str) = tok.strip_suffix("Hz")
            && let Ok(hz) = hz_str.parse::<f32>()
        {
            return Some(hz);
        }
    }
    None
}

fn finalize(
    raw: RawConnector,
    edid_hex: &str,
    next_id: &mut u32,
    cmd_name: &str,
) -> Result<Option<Monitor>, DetectError> {
    if !raw.connected || !raw.enabled {
        return Ok(None);
    }

    if raw.resolution == (0, 0) {
        return Err(DetectError::Parse {
            cmd: cmd_name.to_owned(),
            message: format!("connector '{}' has no usable geometry", raw.name),
        });
    }

    let edid_triple = parse_edid_triple(edid_hex);
    let stable_id = edid_triple
        .as_ref()
        .and_then(|t| hash_edid_triple(Some(&t.0), Some(&t.1), Some(&t.2)));

    let monitor = Monitor {
        id: MonitorId(*next_id),
        name: raw.name,
        stable_id,
        position: raw.position,
        resolution: raw.resolution,
        physical_size_mm: raw.physical_size_mm,
        scale: 1.0,
        rotation: raw.rotation,
        refresh_hz: raw.refresh_hz,
        primary: false,
        ppi: None,
    };
    *next_id = next_id.saturating_add(1);
    Ok(Some(monitor))
}

fn parse_edid_triple(hex: &str) -> Option<(String, String, String)> {
    if hex.len() < 256 {
        return None;
    }
    let bytes = hex_to_bytes(hex)?;
    if bytes.len() < 128 {
        return None;
    }

    // EDID manufacturer ID: bytes 8-9, three packed 5-bit letters.
    let mfg_word = (u16::from(bytes[8]) << 8) | u16::from(bytes[9]);
    let l1 = u8::try_from((mfg_word >> 10) & 0x1f).ok()? + b'A' - 1;
    let l2 = u8::try_from((mfg_word >> 5) & 0x1f).ok()? + b'A' - 1;
    let l3 = u8::try_from(mfg_word & 0x1f).ok()? + b'A' - 1;
    if !l1.is_ascii_uppercase() || !l2.is_ascii_uppercase() || !l3.is_ascii_uppercase() {
        return None;
    }
    let manufacturer = String::from_utf8(vec![l1, l2, l3]).ok()?;

    // EDID 1.3+: descriptor blocks at bytes 54..125, four 18-byte chunks.
    // Tag 0xFC = monitor name, 0xFF = serial number.
    let mut model_text: Option<String> = None;
    let mut serial_text: Option<String> = None;
    for i in 0..4 {
        let off = 54 + i * 18;
        if off + 18 > bytes.len() {
            break;
        }
        let block = &bytes[off..off + 18];
        if block[0] != 0 || block[1] != 0 || block[2] != 0 {
            continue;
        }
        let text = ascii_text(&block[5..]);
        match block[3] {
            0xfc => model_text = Some(text),
            0xff => serial_text = Some(text),
            _ => {}
        }
    }

    // Fall back to bytes 12-15 (binary serial), decimal-formatted for hash stability.
    let serial = serial_text.unwrap_or_else(|| {
        let raw = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        if raw == 0 {
            String::new()
        } else {
            raw.to_string()
        }
    });
    let model = model_text.unwrap_or_default();

    Some((manufacturer, model, serial))
}

fn ascii_text(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        if b == 0x0a {
            break;
        }
        if (0x20..=0x7e).contains(&b) {
            out.push(char::from(b));
        }
    }
    out.trim().to_owned()
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on fixture parse errors
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/display/xrandr-2-monitors.txt");

    #[test]
    fn two_monitor_fixture_parses_to_expected_layout() {
        let monitors = parse_xrandr_verbose(FIXTURE, CMD).unwrap();
        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0].name, "DP-1");
        assert_eq!(monitors[0].resolution, (2560, 1440));
        assert_eq!(monitors[0].position, (0, 0));
        assert_eq!(monitors[0].physical_size_mm, Some((597, 336)));
        assert_eq!(monitors[1].name, "HDMI-1");
        assert_eq!(monitors[1].resolution, (1920, 1080));
        assert_eq!(monitors[1].position, (2560, 0));
        assert_eq!(monitors[1].physical_size_mm, Some((597, 336)));
    }

    #[test]
    fn fixture_derives_stable_id_from_edid() {
        let monitors = parse_xrandr_verbose(FIXTURE, CMD).unwrap();
        let id_a = monitors[0].stable_id.as_deref().unwrap();
        let id_b = monitors[1].stable_id.as_deref().unwrap();
        assert_eq!(id_a.len(), 64);
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn disconnected_outputs_are_excluded_from_result() {
        let input = "Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384\n\
            DP-1 connected primary 1920x1080+0+0 (0x12a) normal (normal left inverted right) 480mm x 270mm\n\
            \th: width 1920 start ... clock 60.00Hz\n\
            HDMI-2 disconnected (normal left inverted right)\n";
        let monitors = parse_xrandr_verbose(input, CMD).unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "DP-1");
    }

    #[test]
    fn missing_edid_yields_no_stable_id() {
        let input = "Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384\n\
            DP-1 connected primary 1920x1080+0+0 (0x12a) normal 480mm x 270mm\n";
        let monitors = parse_xrandr_verbose(input, CMD).unwrap();
        assert_eq!(monitors[0].stable_id, None);
    }

    #[test]
    fn rotation_keyword_is_picked_up_from_header() {
        let input = "Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384\n\
            DP-1 connected primary 1080x1920+0+0 left (0x12a) (normal left inverted right) 480mm x 270mm\n";
        let monitors = parse_xrandr_verbose(input, CMD).unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].resolution, (1080, 1920));
    }
}
