// Bezel-gap detection + normalisation. Pairs are "immediate" — A is to the
// left of B with overlap and no third monitor strictly between.

import type { MonitorOverride } from '$lib/stores/canvas-view.svelte';
import type { PreviewMonitor } from './projection';

const GAP_EPS_MM = 0.5;

export type GapPair = {
  a: PreviewMonitor;
  b: PreviewMonitor;
  gapMm: number;
};

export function yOverlap(a: PreviewMonitor, b: PreviewMonitor): boolean {
  return a.yMm < b.yMm + b.hMm - GAP_EPS_MM && b.yMm < a.yMm + a.hMm - GAP_EPS_MM;
}

export function xOverlap(a: PreviewMonitor, b: PreviewMonitor): boolean {
  return a.xMm < b.xMm + b.wMm - GAP_EPS_MM && b.xMm < a.xMm + a.wMm - GAP_EPS_MM;
}

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

export function uniformGap(pairs: GapPair[], tolMm = GAP_EPS_MM): number | null {
  if (pairs.length === 0) return null;
  const first = pairs[0]?.gapMm;
  if (first === undefined) return null;
  for (const p of pairs) {
    if (Math.abs(p.gapMm - first) > tolMm) return null;
  }
  return first;
}

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

export function normaliseAxis(
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
    const ex = next[m.id] ?? { xMm: m.xMm, yMm: m.yMm };
    next[m.id] = { ...ex, xMm: w.x, yMm: w.y };
  }
  return next;
}
