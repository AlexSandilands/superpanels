// mm-space layout helpers for the new preview canvas. The detected monitor
// list arrives with (resolution, physical_size_mm, position-px) — we lay them
// out in mm rows at compositor order, with horizontal/vertical bezel gaps
// between row members. The user can then drag individual monitors to override
// these positions; overrides are stored in `canvasView.overrides` keyed by a
// stable id.

import type { Monitor } from '$lib/api';
import type { MonitorOverride } from '$lib/stores/canvasView.svelte';

const FALLBACK_DPI = 96;

export type PreviewMonitor = {
  id: string;
  name: string;
  model: string;
  refreshHz: number | null;
  primary: boolean;
  rotation: 0 | 90 | 180 | 270;
  // Native (un-rotated) physical dims in mm.
  nativeWmm: number;
  nativeHmm: number;
  // Native pixel resolution.
  nativePxW: number;
  nativePxH: number;
  // Effective (post-rotation) mm and px.
  wMm: number;
  hMm: number;
  pxW: number;
  pxH: number;
  // Position of top-left in mm space.
  xMm: number;
  yMm: number;
  // True when physical_size_mm is missing (we fall back to 96 DPI).
  missing: boolean;
};

export function stableId(m: Monitor): string {
  return m.stable_id?.length ? m.stable_id : m.name;
}

export function rotationDeg(m: Monitor): 0 | 90 | 180 | 270 {
  switch (m.rotation) {
    case 'right':
      return 90;
    case 'inverted':
      return 180;
    case 'left':
      return 270;
    default:
      return 0;
  }
}

function nativeMm(m: Monitor): { w: number; h: number; missing: boolean } {
  if (m.physical_size_mm) {
    return { w: m.physical_size_mm[0], h: m.physical_size_mm[1], missing: false };
  }
  return {
    w: (m.resolution[0] / FALLBACK_DPI) * 25.4,
    h: (m.resolution[1] / FALLBACK_DPI) * 25.4,
    missing: true,
  };
}

// Default mm positions — lay monitors out in compositor-reported order along
// rows, separated by bezel gaps. Returns one override per monitor.
export function defaultOverrides(
  monitors: Monitor[],
  bezelHmm: number,
): Record<string, MonitorOverride> {
  const out: Record<string, MonitorOverride> = {};
  // Sort by reported position so the row order is reproducible.
  const sorted = [...monitors].sort(
    (a, b) => a.position[1] - b.position[1] || a.position[0] - b.position[0],
  );
  let cursorX = 0;
  for (const m of sorted) {
    const rot = rotationDeg(m);
    const nm = nativeMm(m);
    const rotated = rot === 90 || rot === 270;
    const wMm = rotated ? nm.h : nm.w;
    out[stableId(m)] = { xMm: cursorX, yMm: 0, rotation: rot };
    cursorX += wMm + bezelHmm;
  }
  return out;
}

// Build the live preview-monitor list by combining detection + overrides.
export function buildPreviewMonitors(
  monitors: Monitor[],
  overrides: Record<string, MonitorOverride>,
): PreviewMonitor[] {
  return monitors.map((m) => {
    const id = stableId(m);
    const nm = nativeMm(m);
    const ov = overrides[id] ?? { xMm: 0, yMm: 0, rotation: rotationDeg(m) };
    const rotated = ov.rotation === 90 || ov.rotation === 270;
    return {
      id,
      name: m.name,
      model: '',
      refreshHz: m.refresh_hz,
      primary: m.primary,
      rotation: ov.rotation,
      nativeWmm: nm.w,
      nativeHmm: nm.h,
      nativePxW: m.resolution[0],
      nativePxH: m.resolution[1],
      wMm: rotated ? nm.h : nm.w,
      hMm: rotated ? nm.w : nm.h,
      pxW: rotated ? m.resolution[1] : m.resolution[0],
      pxH: rotated ? m.resolution[0] : m.resolution[1],
      xMm: ov.xMm,
      yMm: ov.yMm,
      missing: nm.missing,
    };
  });
}

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

const GAP_EPS_MM = 0.5;

export type GapPair = {
  a: PreviewMonitor;
  b: PreviewMonitor;
  gapMm: number;
};

function yOverlap(a: PreviewMonitor, b: PreviewMonitor): boolean {
  return a.yMm < b.yMm + b.hMm - GAP_EPS_MM && b.yMm < a.yMm + a.hMm - GAP_EPS_MM;
}

function xOverlap(a: PreviewMonitor, b: PreviewMonitor): boolean {
  return a.xMm < b.xMm + b.wMm - GAP_EPS_MM && b.xMm < a.xMm + a.wMm - GAP_EPS_MM;
}

// Immediate horizontal-neighbour pairs: A is to the left of B with y-overlap
// and no third monitor lies strictly between them in x while overlapping both.
export function hNeighbourPairs(ms: PreviewMonitor[]): GapPair[] {
  const out: GapPair[] = [];
  for (const a of ms) {
    for (const b of ms) {
      if (b.id === a.id) continue;
      if (b.xMm < a.xMm + a.wMm - GAP_EPS_MM) continue;
      if (!yOverlap(a, b)) continue;
      let between = false;
      for (const c of ms) {
        if (c.id === a.id || c.id === b.id) continue;
        if (c.xMm < a.xMm + a.wMm - GAP_EPS_MM) continue;
        if (c.xMm + c.wMm > b.xMm + GAP_EPS_MM) continue;
        if (yOverlap(a, c) && yOverlap(b, c)) {
          between = true;
          break;
        }
      }
      if (!between) out.push({ a, b, gapMm: b.xMm - (a.xMm + a.wMm) });
    }
  }
  return out;
}

export function vNeighbourPairs(ms: PreviewMonitor[]): GapPair[] {
  const out: GapPair[] = [];
  for (const a of ms) {
    for (const b of ms) {
      if (b.id === a.id) continue;
      if (b.yMm < a.yMm + a.hMm - GAP_EPS_MM) continue;
      if (!xOverlap(a, b)) continue;
      let between = false;
      for (const c of ms) {
        if (c.id === a.id || c.id === b.id) continue;
        if (c.yMm < a.yMm + a.hMm - GAP_EPS_MM) continue;
        if (c.yMm + c.hMm > b.yMm + GAP_EPS_MM) continue;
        if (xOverlap(a, c) && xOverlap(b, c)) {
          between = true;
          break;
        }
      }
      if (!between) out.push({ a, b, gapMm: b.yMm - (a.yMm + a.hMm) });
    }
  }
  return out;
}

// Single uniform gap if all pairs agree within tolerance, else null.
export function uniformGap(pairs: GapPair[], tolMm = GAP_EPS_MM): number | null {
  if (pairs.length === 0) return null;
  const first = pairs[0]?.gapMm;
  if (first === undefined) return null;
  for (const p of pairs) {
    if (Math.abs(p.gapMm - first) > tolMm) return null;
  }
  return first;
}

// Re-space monitors so every immediate H-neighbour pair has `targetMm` between
// them. Each connected H-chain keeps its leftmost head in place; subsequent
// monitors are pushed to head.right + target. Y is untouched.
export function normaliseHGaps(
  monitors: PreviewMonitor[],
  overrides: Record<string, MonitorOverride>,
  targetMm: number,
): Record<string, MonitorOverride> {
  return normaliseAxis(monitors, overrides, targetMm, hNeighbourPairs, 'x');
}

export function normaliseVGaps(
  monitors: PreviewMonitor[],
  overrides: Record<string, MonitorOverride>,
  targetMm: number,
): Record<string, MonitorOverride> {
  return normaliseAxis(monitors, overrides, targetMm, vNeighbourPairs, 'y');
}

function normaliseAxis(
  monitors: PreviewMonitor[],
  overrides: Record<string, MonitorOverride>,
  targetMm: number,
  pairsOf: (ms: PreviewMonitor[]) => GapPair[],
  axis: 'x' | 'y',
): Record<string, MonitorOverride> {
  type W = { x: number; y: number; w: number; h: number };
  const work = new Map<string, W>();
  for (const m of monitors) {
    work.set(m.id, { x: m.xMm, y: m.yMm, w: m.wMm, h: m.hMm });
  }

  const pairs = pairsOf(monitors);
  const lefts = new Map<string, string[]>();
  for (const m of monitors) lefts.set(m.id, []);
  for (const p of pairs) lefts.get(p.b.id)?.push(p.a.id);

  const settled = new Set<string>();
  for (const m of monitors) {
    if ((lefts.get(m.id) ?? []).length === 0) settled.add(m.id);
  }

  let progress = true;
  while (progress) {
    progress = false;
    for (const m of monitors) {
      if (settled.has(m.id)) continue;
      const ls = lefts.get(m.id) ?? [];
      if (!ls.every((l) => settled.has(l))) continue;
      let edge = -Infinity;
      for (const l of ls) {
        const lw = work.get(l);
        if (!lw) continue;
        const e = axis === 'x' ? lw.x + lw.w : lw.y + lw.h;
        if (e > edge) edge = e;
      }
      const cur = work.get(m.id);
      if (cur && edge !== -Infinity) {
        if (axis === 'x') cur.x = edge + targetMm;
        else cur.y = edge + targetMm;
      }
      settled.add(m.id);
      progress = true;
    }
  }

  const next: Record<string, MonitorOverride> = { ...overrides };
  for (const m of monitors) {
    const w = work.get(m.id);
    if (!w) continue;
    const ex = next[m.id] ?? { xMm: m.xMm, yMm: m.yMm, rotation: m.rotation };
    next[m.id] = { ...ex, xMm: w.x, yMm: w.y };
  }
  return next;
}

// Aspect-preserving image rect that "covers" the monitor union (mm units).
export function coverImageRect(
  monitors: PreviewMonitor[],
  imageAspect: number,
): { offsetMmX: number; offsetMmY: number; widthMm: number; heightMm: number } {
  const bb = bbox(monitors);
  let w = bb.w;
  let h = w / imageAspect;
  if (h < bb.h) {
    h = bb.h;
    w = h * imageAspect;
  }
  return {
    offsetMmX: bb.x + bb.w / 2 - w / 2,
    offsetMmY: bb.y + bb.h / 2 - h / 2,
    widthMm: w,
    heightMm: h,
  };
}
