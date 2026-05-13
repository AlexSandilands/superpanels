// Topology rectangles for the small SVG previews shown in the profile manager.
// Joins authored placements (`monitor_state`) with currently-detected
// monitors so we can use real physical sizes when the setup matches; falls
// back to a default rect when a profile was authored against monitors that
// aren't connected. Rotation comes from the live `Monitor.rotation` (OS),
// not from the placement.

import type { Monitor } from './api';
import type { MonitorPlacement } from './types/MonitorPlacement';

export type TopologyRect = { x: number; y: number; w: number; h: number };

const FALLBACK_W_MM = 600;
const FALLBACK_H_MM = 340;

export function topologyRects(
  monitorState: Record<string, MonitorPlacement>,
  monitors: Monitor[],
): TopologyRect[] {
  const out: TopologyRect[] = [];
  for (const [id, p] of Object.entries(monitorState)) {
    const m = monitors.find((mm) => mm.stable_id === id || mm.name === id);
    let w = FALLBACK_W_MM;
    let h = FALLBACK_H_MM;
    if (m?.physical_size_mm) {
      [w, h] = m.physical_size_mm;
    }
    if (m?.rotation === 'left' || m?.rotation === 'right') {
      [w, h] = [h, w];
    }
    out.push({ x: p.x_mm, y: p.y_mm, w, h });
  }
  return out;
}
