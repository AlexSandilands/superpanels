// mm-space projection of compositor-detected monitors. Computes default
// row-layout positions, builds the live `PreviewMonitor[]` from detection +
// overrides, and the cover-fit image rect. SPEC §4 (bezel math).

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

export function nativeMm(m: Monitor): { w: number; h: number; missing: boolean } {
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

// Aspect-preserving image rect that "covers" the monitor union (mm units).
export function coverImageRect(
  monitors: PreviewMonitor[],
  imageAspect: number,
): { offsetMmX: number; offsetMmY: number; widthMm: number; heightMm: number } {
  if (monitors.length === 0) {
    return { offsetMmX: 0, offsetMmY: 0, widthMm: 1000, heightMm: 1000 / imageAspect };
  }
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const m of monitors) {
    if (m.xMm < x0) x0 = m.xMm;
    if (m.yMm < y0) y0 = m.yMm;
    if (m.xMm + m.wMm > x1) x1 = m.xMm + m.wMm;
    if (m.yMm + m.hMm > y1) y1 = m.yMm + m.hMm;
  }
  const bbW = x1 - x0;
  const bbH = y1 - y0;
  let w = bbW;
  let h = w / imageAspect;
  if (h < bbH) {
    h = bbH;
    w = h * imageAspect;
  }
  return {
    offsetMmX: x0 + bbW / 2 - w / 2,
    offsetMmY: y0 + bbH / 2 - h / 2,
    widthMm: w,
    heightMm: h,
  };
}
