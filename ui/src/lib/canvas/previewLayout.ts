// mm-space layout orchestrator. Re-exports projection (positions + cover-fit
// rect) and gaps (neighbour detection + normalisation), plus geometry
// primitives shared by both. SPEC §4.

export type {
  PreviewMonitor,
  // re-exported types
} from './previewLayout/projection';
export {
  buildPreviewMonitors,
  coverImageRect,
  defaultOverrides,
  nativeMm,
  rotationDeg,
  stableId,
} from './previewLayout/projection';
export type { GapPair } from './previewLayout/gaps';
export {
  hNeighbourPairs,
  normaliseAxis,
  normaliseHGaps,
  normaliseVGaps,
  uniformGap,
  vNeighbourPairs,
  xOverlap,
  yOverlap,
} from './previewLayout/gaps';

import type { PreviewMonitor } from './previewLayout/projection';

export type Rect = { x: number; y: number; w: number; h: number };

export function monitorRect(m: PreviewMonitor): Rect {
  return { x: m.xMm, y: m.yMm, w: m.wMm, h: m.hMm };
}

export function bbox(monitors: PreviewMonitor[]): Rect {
  if (monitors.length === 0) return { x: 0, y: 0, w: 1000, h: 600 };
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const m of monitors) {
    const r = monitorRect(m);
    if (r.x < x0) x0 = r.x;
    if (r.y < y0) y0 = r.y;
    if (r.x + r.w > x1) x1 = r.x + r.w;
    if (r.y + r.h > y1) y1 = r.y + r.h;
  }
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

export function totalPixels(monitors: PreviewMonitor[]): { w: number; h: number } {
  let w = 0;
  let h = 0;
  for (const m of monitors) {
    w += m.pxW;
    if (m.pxH > h) h = m.pxH;
  }
  return { w, h };
}
