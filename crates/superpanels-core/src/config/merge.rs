//! Merge `[[monitor]]` config blocks into a detected `Vec<Monitor>` so each
//! monitor picks up its physical mm + derived PPI (`SPEC.md` §14.1).

use crate::display::{Monitor, Rotation};

use super::MonitorConfig;

const MM_PER_INCH: f64 = 25.4;

/// Match priority: `stable_id` first, then `name`.
pub(super) fn merge_monitor_config(blocks: &[MonitorConfig], monitors: &mut [Monitor]) {
    for block in blocks {
        let matched = monitors.iter_mut().find(|m| matches_block(block, m));
        if let Some(m) = matched {
            let (w, h) = (block.physical_mm[0], block.physical_mm[1]);
            m.physical_size_mm = Some((w, h));
            m.ppi = Some(compute_ppi(m.resolution, (w, h), m.rotation));
        }
    }
}

fn matches_block(block: &MonitorConfig, m: &Monitor) -> bool {
    if let (Some(b), Some(d)) = (block.stable_id.as_deref(), m.stable_id.as_deref())
        && b == d
    {
        return true;
    }
    if let (Some(b), d) = (block.name.as_deref(), m.name.as_str())
        && b == d
    {
        return true;
    }
    false
}

fn compute_ppi(resolution: (u32, u32), physical_mm: (f64, f64), rotation: Rotation) -> f64 {
    let (px_w, px_h) = match rotation {
        Rotation::None | Rotation::Inverted => resolution,
        Rotation::Right | Rotation::Left => (resolution.1, resolution.0),
    };
    let (mm_w, mm_h) = physical_mm;
    let ppi_w = f64::from(px_w) / (mm_w / MM_PER_INCH);
    let ppi_h = f64::from(px_h) / (mm_h / MM_PER_INCH);
    f64::midpoint(ppi_w, ppi_h)
}
