// Shared canvas layout types. SPEC §4 / §12.3.

import type { Monitor } from '$lib/api';

export type MonitorRect = {
  monitorIndex: number;
  monitorName: string;
  // Canvas-pixel rectangle of this monitor's visible area.
  x: number;
  y: number;
  w: number;
  h: number;
  // Effective physical size in mm (post-rotation).
  widthMm: number;
  heightMm: number;
  // Native pixel dimensions (post-rotation, what the monitor displays).
  pixelW: number;
  pixelH: number;
  rotated: boolean;
  // True when physical_size_mm is missing — the monitor is drawn from the
  // 96-DPI fallback so the layout still renders.
  missing: boolean;
};

export type BezelBar = {
  // Canvas-pixel rectangle of the bezel gap (drawn over the image).
  x: number;
  y: number;
  w: number;
  h: number;
  orientation: 'vertical' | 'horizontal';
};

export type CanvasLayout = {
  // Total canvas-pixel size of the layout (the source-image-mapped area,
  // including bezel gaps).
  totalW: number;
  totalH: number;
  // Top-left of the layout inside the viewport (centred + padding).
  offsetX: number;
  offsetY: number;
  // mm-to-canvas-pixel scale used for the layout.
  mmToPx: number;
  // Pixel scale of the physical-layout canvas used by Rust crop math.
  coreMmToPx: number;
  monitors: MonitorRect[];
  bezels: BezelBar[];
  // Physical canvas total in mm (informational / for popouts).
  totalMmW: number;
  totalMmH: number;
};

export type LayoutInput = {
  monitors: Monitor[];
  bezelHmm: number;
  bezelVmm: number;
  viewportW: number;
  viewportH: number;
  padding: number;
  // Optional zoom factor applied on top of the fit scale (1.0 default).
  zoom: number;
};
