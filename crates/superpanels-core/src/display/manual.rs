//! `--monitors` manual-override parser (`SPEC.md` §6.2).
//!
//! Each comma-separated entry: `WxH+X+Y[@SCALE][/ROT][?WMMxHMM]`.

use super::{DetectError, Monitor, MonitorId, Rotation};

const CMD: &str = "--monitors";

/// Parse a `--monitors` value into a list of [`Monitor`]s. IDs are assigned
/// sequentially; names are `manual-<i>`.
pub fn parse_manual_monitors(spec: &str) -> Result<Vec<Monitor>, DetectError> {
    if spec.trim().is_empty() {
        return Err(DetectError::Parse {
            cmd: CMD.to_owned(),
            message: "monitor spec is empty".to_owned(),
        });
    }
    let mut monitors = Vec::new();
    for (idx, entry) in spec.split(',').enumerate() {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Err(DetectError::Parse {
                cmd: CMD.to_owned(),
                message: format!("entry {idx} is empty (stray comma?)"),
            });
        }
        monitors.push(parse_one(trimmed, idx)?);
    }
    Ok(monitors)
}

fn parse_one(entry: &str, idx: usize) -> Result<Monitor, DetectError> {
    let id = u32::try_from(idx).map_err(|e| DetectError::Parse {
        cmd: CMD.to_owned(),
        message: format!("monitor index {idx} does not fit in u32: {e}"),
    })?;

    let mut remainder = entry;
    let mut physical_size_mm: Option<(f64, f64)> = None;
    if let Some((head, tail)) = remainder.split_once('?') {
        let (w, h) = parse_size(tail, idx, "physical size")?;
        physical_size_mm = Some((f64::from(w), f64::from(h)));
        remainder = head;
    }
    let mut rotation = Rotation::None;
    if let Some((head, tail)) = remainder.split_once('/') {
        rotation = parse_rotation(tail, idx)?;
        remainder = head;
    }
    let mut scale = 1.0_f64;
    if let Some((head, tail)) = remainder.split_once('@') {
        scale = tail.parse::<f64>().map_err(|e| DetectError::Parse {
            cmd: CMD.to_owned(),
            message: format!("entry {idx}: invalid scale '{tail}': {e}"),
        })?;
        remainder = head;
    }

    let (res_str, x_str, y_str) = split_layout(remainder, idx)?;
    let resolution = parse_size(res_str, idx, "resolution")?;
    let x: i32 = x_str.parse().map_err(|e| DetectError::Parse {
        cmd: CMD.to_owned(),
        message: format!("entry {idx}: invalid x '{x_str}': {e}"),
    })?;
    let y: i32 = y_str.parse().map_err(|e| DetectError::Parse {
        cmd: CMD.to_owned(),
        message: format!("entry {idx}: invalid y '{y_str}': {e}"),
    })?;

    let ppi = physical_size_mm.map(|(w_mm, h_mm)| compute_ppi(resolution, (w_mm, h_mm), rotation));

    Ok(Monitor {
        id: MonitorId(id),
        name: format!("manual-{idx}"),
        stable_id: None,
        position: (x, y),
        resolution,
        physical_size_mm,
        scale,
        rotation,
        refresh_hz: None,
        primary: false,
        ppi,
    })
}

fn split_layout(s: &str, idx: usize) -> Result<(&str, &str, &str), DetectError> {
    // WxH+X+Y; X or Y may be negative — split on the first sign past index 0.
    let res_end = s
        .char_indices()
        .find(|(i, c)| (*c == '+' || *c == '-') && *i > 0)
        .ok_or_else(|| DetectError::Parse {
            cmd: CMD.to_owned(),
            message: format!("entry {idx}: missing position after resolution"),
        })?
        .0;
    let (res_str, rest) = s.split_at(res_end);

    let pos_split = rest[1..]
        .char_indices()
        .find(|(_, c)| *c == '+' || *c == '-')
        .ok_or_else(|| DetectError::Parse {
            cmd: CMD.to_owned(),
            message: format!("entry {idx}: position must be +X+Y or -X-Y"),
        })?
        .0
        + 1;

    let x_str = &rest[..pos_split];
    let y_str = &rest[pos_split..];
    Ok((res_str, x_str, y_str))
}

fn parse_size(s: &str, idx: usize, field: &str) -> Result<(u32, u32), DetectError> {
    let (w, h) = s.split_once('x').ok_or_else(|| DetectError::Parse {
        cmd: CMD.to_owned(),
        message: format!("entry {idx}: {field} '{s}' is not 'WxH'"),
    })?;
    let w: u32 = w.parse().map_err(|e| DetectError::Parse {
        cmd: CMD.to_owned(),
        message: format!("entry {idx}: invalid {field} width '{w}': {e}"),
    })?;
    let h: u32 = h.parse().map_err(|e| DetectError::Parse {
        cmd: CMD.to_owned(),
        message: format!("entry {idx}: invalid {field} height '{h}': {e}"),
    })?;
    Ok((w, h))
}

fn parse_rotation(s: &str, idx: usize) -> Result<Rotation, DetectError> {
    match s.to_ascii_lowercase().as_str() {
        "0" | "none" | "normal" => Ok(Rotation::None),
        "1" | "right" => Ok(Rotation::Right),
        "2" | "inverted" => Ok(Rotation::Inverted),
        "3" | "left" => Ok(Rotation::Left),
        other => Err(DetectError::Parse {
            cmd: CMD.to_owned(),
            message: format!(
                "entry {idx}: unknown rotation '{other}' (expected none|right|inverted|left or 0..=3)"
            ),
        }),
    }
}

fn compute_ppi(resolution: (u32, u32), physical_mm: (f64, f64), rotation: Rotation) -> f64 {
    let (px_w, px_h) = match rotation {
        Rotation::None | Rotation::Inverted => resolution,
        Rotation::Right | Rotation::Left => (resolution.1, resolution.0),
    };
    let (mm_w, mm_h) = physical_mm;
    let ppi_w = f64::from(px_w) / (mm_w / 25.4);
    let ppi_h = f64::from(px_h) / (mm_h / 25.4);
    f64::midpoint(ppi_w, ppi_h)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on parse errors
mod tests {
    use super::*;

    #[test]
    fn layout_only_entry_parses_without_optional_fields() {
        let monitors = parse_manual_monitors("1920x1080+0+0").unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].resolution, (1920, 1080));
        assert_eq!(monitors[0].position, (0, 0));
        assert!((monitors[0].scale - 1.0).abs() < f64::EPSILON);
        assert_eq!(monitors[0].rotation, Rotation::None);
        assert_eq!(monitors[0].physical_size_mm, None);
        assert_eq!(monitors[0].ppi, None);
        assert_eq!(monitors[0].name, "manual-0");
        assert_eq!(monitors[0].stable_id, None);
        assert!(!monitors[0].primary);
    }

    #[test]
    fn full_spec_with_mm_populates_ppi() {
        let monitors = parse_manual_monitors("2560x1440+0+0@1.5/right?597x336").unwrap();
        assert_eq!(monitors.len(), 1);
        let m = &monitors[0];
        assert_eq!(m.resolution, (2560, 1440));
        assert_eq!(m.position, (0, 0));
        assert!((m.scale - 1.5).abs() < f64::EPSILON);
        assert_eq!(m.rotation, Rotation::Right);
        assert_eq!(m.physical_size_mm, Some((597.0, 336.0)));
        let ppi = m.ppi.unwrap();
        assert!(
            ppi > 80.0 && ppi < 130.0,
            "expected reasonable PPI, got {ppi}"
        );
    }

    #[test]
    fn multiple_entries_get_sequential_ids_and_names() {
        let monitors = parse_manual_monitors("1920x1080+0+0,2560x1440+1920+0/inverted").unwrap();
        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0].id, MonitorId(0));
        assert_eq!(monitors[0].name, "manual-0");
        assert_eq!(monitors[1].id, MonitorId(1));
        assert_eq!(monitors[1].name, "manual-1");
        assert_eq!(monitors[1].rotation, Rotation::Inverted);
        assert_eq!(monitors[1].position, (1920, 0));
    }

    #[test]
    fn malformed_entry_returns_parse_error() {
        let err = parse_manual_monitors("not-a-monitor").unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));

        let err = parse_manual_monitors("").unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));

        let err = parse_manual_monitors("1920x1080+0+0,").unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));

        let err = parse_manual_monitors("1920x1080+0+0/sideways").unwrap_err();
        assert!(matches!(err, DetectError::Parse { .. }));
    }
}
