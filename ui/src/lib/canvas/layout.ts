// Canvas layout math. SPEC §4 ("the bezel & layout math") simplified for
// display: we don't need source-image pixel mapping here, only the canvas
// rectangles to draw. The Rust crop algorithm runs on Apply via IPC.
//
// Algorithm summary (SPEC §4.4 minus the source-image-pixel step):
//   1. Build effective monitors (apply rotation to mm + pixel dims).
//   2. Group into rows by y-overlap of the logical desktop.
//   3. Within each row, sort by x and lay out at cumulative widthMm + bezels.
//   4. Stack rows vertically with vertical bezel gaps between them.
//   5. Fit the resulting mm canvas into the viewport with `padding` and `zoom`.

import type { Monitor } from '$lib/api';
import type { BezelBar, CanvasLayout, LayoutInput, MonitorRect } from './types';

const FALLBACK_DPI = 96;

type Eff = {
  index: number;
  monitor: Monitor;
  widthMm: number;
  heightMm: number;
  pixelW: number;
  pixelH: number;
  rotated: boolean;
  missing: boolean;
  posX: number;
  posY: number;
};

export function computeLayout(input: LayoutInput): CanvasLayout {
  const { monitors, viewportW, viewportH, padding, zoom } = input;
  if (monitors.length === 0 || viewportW <= 0 || viewportH <= 0) {
    return emptyLayout(viewportW, viewportH);
  }

  const effs = monitors.map((m, i) => effective(m, i));
  const rows = groupRows(effs);

  const bezelH = Math.max(0, input.bezelHmm);
  const bezelV = Math.max(0, input.bezelVmm);

  // Per-row mm geometry: sum widths + (n-1) horizontal bezels; height = max heightMm.
  const rowGeom = rows.map((row) => {
    const widthMm =
      row.reduce((acc, e) => acc + e.widthMm, 0) + Math.max(0, row.length - 1) * bezelH;
    const heightMm = row.reduce((acc, e) => Math.max(acc, e.heightMm), 0);
    return { widthMm, heightMm };
  });

  const totalMmW = rowGeom.reduce((acc, r) => Math.max(acc, r.widthMm), 0);
  const totalMmH =
    rowGeom.reduce((acc, r) => acc + r.heightMm, 0) + Math.max(0, rowGeom.length - 1) * bezelV;

  if (totalMmW <= 0 || totalMmH <= 0) {
    return emptyLayout(viewportW, viewportH);
  }

  const innerW = Math.max(1, viewportW - padding * 2);
  const innerH = Math.max(1, viewportH - padding * 2);
  const fitScale = Math.min(innerW / totalMmW, innerH / totalMmH);
  const mmToPx = Math.max(0.001, fitScale * Math.max(0.1, zoom));
  const coreMmToPx = computeCoreMmToPx(effs);

  const totalW = totalMmW * mmToPx;
  const totalH = totalMmH * mmToPx;
  const offsetX = (viewportW - totalW) / 2;
  const offsetY = (viewportH - totalH) / 2;

  const monitorRects: MonitorRect[] = [];
  const bezels: BezelBar[] = [];

  let cumulativeYmm = 0;
  rows.forEach((row, rowIdx) => {
    const geom = rowGeom[rowIdx];
    if (!geom) return;
    const rowYmm = cumulativeYmm;
    let cumulativeXmm = 0;
    row.forEach((eff, colIdx) => {
      const xMm = cumulativeXmm;
      monitorRects.push({
        monitorIndex: eff.index,
        monitorName: eff.monitor.name,
        x: offsetX + xMm * mmToPx,
        y: offsetY + rowYmm * mmToPx,
        w: eff.widthMm * mmToPx,
        h: eff.heightMm * mmToPx,
        widthMm: eff.widthMm,
        heightMm: eff.heightMm,
        pixelW: eff.pixelW,
        pixelH: eff.pixelH,
        rotated: eff.rotated,
        missing: eff.missing,
      });
      cumulativeXmm += eff.widthMm;
      if (colIdx < row.length - 1 && bezelH > 0) {
        bezels.push({
          x: offsetX + cumulativeXmm * mmToPx,
          y: offsetY + rowYmm * mmToPx,
          w: bezelH * mmToPx,
          h: geom.heightMm * mmToPx,
          orientation: 'vertical',
        });
        cumulativeXmm += bezelH;
      }
    });
    cumulativeYmm += geom.heightMm;
    if (rowIdx < rows.length - 1 && bezelV > 0) {
      bezels.push({
        x: offsetX,
        y: offsetY + cumulativeYmm * mmToPx,
        w: totalMmW * mmToPx,
        h: bezelV * mmToPx,
        orientation: 'horizontal',
      });
      cumulativeYmm += bezelV;
    }
  });

  return {
    totalW,
    totalH,
    offsetX,
    offsetY,
    mmToPx,
    coreMmToPx,
    monitors: monitorRects,
    bezels,
    totalMmW,
    totalMmH,
  };
}

function effective(m: Monitor, index: number): Eff {
  const rotated = m.rotation === 'left' || m.rotation === 'right';
  const mm = m.physical_size_mm;
  const pxW = rotated ? m.resolution[1] : m.resolution[0];
  const pxH = rotated ? m.resolution[0] : m.resolution[1];
  // Native-orientation mm; rotation swaps the axes.
  const nativeW = mm ? mm[0] : (m.resolution[0] / FALLBACK_DPI) * 25.4;
  const nativeH = mm ? mm[1] : (m.resolution[1] / FALLBACK_DPI) * 25.4;
  return {
    index,
    monitor: m,
    widthMm: rotated ? nativeH : nativeW,
    heightMm: rotated ? nativeW : nativeH,
    pixelW: pxW,
    pixelH: pxH,
    rotated,
    missing: mm === null,
    posX: m.position[0],
    posY: m.position[1],
  };
}

function groupRows(effs: Eff[]): Eff[][] {
  // Group by y-overlap of the logical desktop. Sort by (y, x) first (SPEC §4.4 step 1).
  const sorted = [...effs].sort((a, b) => a.posY - b.posY || a.posX - b.posX);
  const rows: Eff[][] = [];
  for (const eff of sorted) {
    const rect = logicalRect(eff);
    const target = rows.find((row) =>
      row.some((other) => verticallyOverlaps(rect, logicalRect(other))),
    );
    if (target) {
      target.push(eff);
    } else {
      rows.push([eff]);
    }
  }
  for (const row of rows) {
    row.sort((a, b) => a.posX - b.posX);
  }
  rows.sort((a, b) => {
    const ay = a[0]?.posY ?? 0;
    const by = b[0]?.posY ?? 0;
    return ay - by;
  });
  return rows;
}

type LogicalRect = { x: number; y: number; w: number; h: number };

function logicalRect(eff: Eff): LogicalRect {
  return {
    x: eff.posX,
    y: eff.posY,
    w: eff.pixelW,
    h: eff.pixelH,
  };
}

function verticallyOverlaps(a: LogicalRect, b: LogicalRect): boolean {
  return a.y < b.y + b.h && b.y < a.y + a.h;
}

function emptyLayout(viewportW: number, viewportH: number): CanvasLayout {
  return {
    totalW: 0,
    totalH: 0,
    offsetX: viewportW / 2,
    offsetY: viewportH / 2,
    mmToPx: 1,
    coreMmToPx: 1,
    monitors: [],
    bezels: [],
    totalMmW: 0,
    totalMmH: 0,
  };
}

function computeCoreMmToPx(effs: Eff[]): number {
  const referencePpi = effs.reduce((max, e) => {
    const ppi = e.pixelW / (e.widthMm / 25.4);
    return Math.max(max, Number.isFinite(ppi) ? ppi : 0);
  }, 0);
  return referencePpi > 0 ? referencePpi / 25.4 : 1;
}

// Serialise a layout to a stable, diff-friendly object. Used by the visual
// regression check noted in the Phase 4a plan §4a.2.
export function serialiseLayout(layout: CanvasLayout): Record<string, unknown> {
  return {
    totalW: round(layout.totalW),
    totalH: round(layout.totalH),
    mmToPx: round(layout.mmToPx, 4),
    coreMmToPx: round(layout.coreMmToPx, 4),
    totalMmW: round(layout.totalMmW),
    totalMmH: round(layout.totalMmH),
    monitors: layout.monitors.map((m) => ({
      index: m.monitorIndex,
      name: m.monitorName,
      x: round(m.x),
      y: round(m.y),
      w: round(m.w),
      h: round(m.h),
      widthMm: round(m.widthMm),
      heightMm: round(m.heightMm),
      pixelW: m.pixelW,
      pixelH: m.pixelH,
      rotated: m.rotated,
      missing: m.missing,
    })),
    bezels: layout.bezels.map((b) => ({
      x: round(b.x),
      y: round(b.y),
      w: round(b.w),
      h: round(b.h),
      orientation: b.orientation,
    })),
  };
}

function round(n: number, digits = 2): number {
  const factor = 10 ** digits;
  return Math.round(n * factor) / factor;
}
