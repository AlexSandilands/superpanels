// Phase 4c free-positioning canvas (SPEC §12.3). Seven-layer compositing:
//   1. Background.
//   2. Image rectangle, free-positioned across the entire canvas.
//   3. Off-monitor dim overlay (toggleable).
//   4. Per-monitor black backing (where the user will see letterbox padding).
//   5. Image redrawn clipped to the union of monitor rects (full alpha).
//   6. Bezel bars between monitors.
//   7. Monitor outlines + labels + (optional) resize handles.
//
// Pure rendering — no interactivity, no DOM lookups. The caller passes a 2D
// context, the layout, and the current draw options.

import type { CanvasLayout, MonitorRect } from './types';

export type DrawOptions = {
  // Pixel ratio applied for HiDPI sharpness.
  dpr: number;
  // Logical canvas dimensions (CSS px).
  viewportW: number;
  viewportH: number;
  // Image to paint as the wallpaper. `null` = empty state (just outlines).
  image: CanvasImageSource | null;
  // Source image natural dimensions (unscaled).
  imageW: number;
  imageH: number;
  // Image-rectangle top-left offset in *display* px.
  offsetX: number;
  offsetY: number;
  // FitMode — used only when `imageSizeDisplayPx` is null (legacy path).
  fit: 'fill' | 'fit' | 'stretch' | 'center';
  // Explicit image rectangle size in *display* px. `null` = use FitMode.
  imageSizeDisplayPx: [number, number] | null;
  // Index of the monitor under the cursor (highlight) — `null` when none.
  hoverIndex: number | null;
  // Labels visible at this zoom level. Disable when very zoomed-out to avoid clutter.
  showLabels: boolean;
  // Dim the off-monitor portion of the image (toggleable via `D` key).
  dim: boolean;
  // Show corner resize handles around the image rectangle.
  showResizeHandles: boolean;
};

const COLOR_BG = '#0b1220';
const COLOR_DIM = 'rgba(0, 0, 0, 0.55)';
const COLOR_BEZEL = '#0b1220';
const COLOR_OUTLINE = 'rgba(96, 165, 250, 0.95)';
const COLOR_OUTLINE_HOVER = 'rgba(125, 211, 252, 1)';
const COLOR_OUTLINE_MISSING = 'rgba(245, 158, 11, 0.9)';
const COLOR_LABEL = '#e2e8f0';
const COLOR_LABEL_SECONDARY = '#94a3b8';
const COLOR_LABEL_MISSING = '#fbbf24';
const COLOR_RESIZE_HANDLE = '#f8fafc';
const COLOR_RESIZE_HANDLE_BORDER = '#0b1220';
const RESIZE_HANDLE_PX = 12;

export function drawCanvasLayers(
  ctx: CanvasRenderingContext2D,
  layout: CanvasLayout,
  opts: DrawOptions,
): void {
  const { dpr, viewportW, viewportH } = opts;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  // 1. Background.
  ctx.fillStyle = COLOR_BG;
  ctx.fillRect(0, 0, viewportW, viewportH);

  if (layout.monitors.length === 0) {
    ctx.restore();
    return;
  }

  const imgRect = computeImageRect(layout, opts);

  // 2. Free-floating image rectangle across the whole canvas.
  drawImageRect(ctx, imgRect, opts, opts.dim ? 0.4 : 1);

  // 3. Off-monitor dim — drawing a translucent black over the off-monitor
  //    region effectively darkens the already-painted image there. We then
  //    redraw the on-monitor image at full alpha in step 5, so the visible
  //    result is "dim outside the monitors, full inside".
  if (opts.dim) {
    drawOffMonitorDim(ctx, layout, viewportW, viewportH);
  }

  // 4. Per-monitor backing — black fill at each monitor's rect. This is what
  //    the user will see in regions the image rectangle doesn't cover.
  ctx.fillStyle = '#000';
  for (const m of layout.monitors) {
    ctx.fillRect(m.x, m.y, m.w, m.h);
  }

  // 5. Image inside monitors, full alpha.
  if (imgRect && opts.image) {
    drawImageInsideMonitors(ctx, layout, imgRect, opts);
  }

  // 6. Bezel bars (the gap between monitors physically blocks the image).
  ctx.fillStyle = COLOR_BEZEL;
  for (const b of layout.bezels) {
    ctx.fillRect(b.x, b.y, b.w, b.h);
  }

  // 7. Monitor outlines + labels + resize handles.
  for (const m of layout.monitors) {
    drawMonitorChrome(ctx, m, opts);
  }
  if (opts.showResizeHandles && imgRect && imgRect.w > 0 && imgRect.h > 0) {
    drawResizeHandles(ctx, imgRect);
  }

  ctx.restore();
}

type ImageRect = { x: number; y: number; w: number; h: number };

export function computeImageRect(layout: CanvasLayout, opts: DrawOptions): ImageRect | null {
  const { imageW, imageH, offsetX, offsetY, fit, imageSizeDisplayPx } = opts;
  if (imageW <= 0 || imageH <= 0) return null;
  if (imageSizeDisplayPx) {
    const [w, h] = imageSizeDisplayPx;
    return { x: offsetX, y: offsetY, w, h };
  }
  // Legacy FitMode-derived placement; mirrors the Phase 4a math so saved
  // profiles without `image_size_px` render identically to before.
  const placement = placeImage(fit, layout.totalW, layout.totalH, imageW, imageH);
  return {
    x: layout.offsetX + placement.x + offsetX,
    y: layout.offsetY + placement.y + offsetY,
    w: placement.w,
    h: placement.h,
  };
}

function drawImageRect(
  ctx: CanvasRenderingContext2D,
  imgRect: ImageRect | null,
  opts: DrawOptions,
  alpha: number,
): void {
  if (!imgRect || !opts.image) return;
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.drawImage(opts.image, imgRect.x, imgRect.y, imgRect.w, imgRect.h);
  ctx.restore();
}

function drawOffMonitorDim(
  ctx: CanvasRenderingContext2D,
  layout: CanvasLayout,
  viewportW: number,
  viewportH: number,
): void {
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, 0, viewportW, viewportH);
  for (const m of layout.monitors) {
    ctx.rect(m.x, m.y, m.w, m.h);
  }
  ctx.fillStyle = COLOR_DIM;
  ctx.fill('evenodd');
  ctx.restore();
}

function drawImageInsideMonitors(
  ctx: CanvasRenderingContext2D,
  layout: CanvasLayout,
  imgRect: ImageRect,
  opts: DrawOptions,
): void {
  if (!opts.image) return;
  ctx.save();
  ctx.beginPath();
  for (const m of layout.monitors) {
    ctx.rect(m.x, m.y, m.w, m.h);
  }
  ctx.clip();
  ctx.globalAlpha = 1;
  ctx.drawImage(opts.image, imgRect.x, imgRect.y, imgRect.w, imgRect.h);
  ctx.restore();
}

type Placement = { x: number; y: number; w: number; h: number };

function placeImage(
  fit: DrawOptions['fit'],
  canvasW: number,
  canvasH: number,
  imageW: number,
  imageH: number,
): Placement {
  if (fit === 'stretch') {
    return { x: 0, y: 0, w: canvasW, h: canvasH };
  }
  const canvasRatio = canvasW / canvasH;
  const imageRatio = imageW / imageH;
  if (fit === 'center') {
    return {
      x: (canvasW - imageW) / 2,
      y: (canvasH - imageH) / 2,
      w: imageW,
      h: imageH,
    };
  }
  // fill (default) or fit
  const wantsCover = fit === 'fill';
  const useWidth = wantsCover ? imageRatio < canvasRatio : imageRatio > canvasRatio;
  const w = useWidth ? canvasW : canvasH * imageRatio;
  const h = useWidth ? canvasW / imageRatio : canvasH;
  return { x: (canvasW - w) / 2, y: (canvasH - h) / 2, w, h };
}

function drawMonitorChrome(ctx: CanvasRenderingContext2D, m: MonitorRect, opts: DrawOptions): void {
  const isHover = opts.hoverIndex === m.monitorIndex;
  const stroke = m.missing ? COLOR_OUTLINE_MISSING : isHover ? COLOR_OUTLINE_HOVER : COLOR_OUTLINE;
  // Phase 4c: the monitor sits on top of the image, so the outline carries the
  // visual weight of "this is a screen". Heavier than Phase 4a's 1.25/2 px.
  ctx.lineWidth = isHover ? 2.5 : 1.75;
  ctx.strokeStyle = stroke;
  ctx.strokeRect(m.x + 0.5, m.y + 0.5, m.w - 1, m.h - 1);

  if (isHover) {
    ctx.save();
    ctx.shadowColor = COLOR_OUTLINE_HOVER;
    ctx.shadowBlur = 12;
    ctx.strokeStyle = COLOR_OUTLINE_HOVER;
    ctx.lineWidth = 1;
    ctx.strokeRect(m.x + 0.5, m.y + 0.5, m.w - 1, m.h - 1);
    ctx.restore();
  }

  if (!opts.showLabels || m.w < 70 || m.h < 36) {
    return;
  }
  drawLabel(ctx, m);
}

function drawLabel(ctx: CanvasRenderingContext2D, m: MonitorRect): void {
  ctx.font = '600 12px ui-sans-serif, system-ui, sans-serif';
  ctx.textBaseline = 'top';
  ctx.fillStyle = COLOR_LABEL;
  ctx.fillText(m.monitorName, m.x + 8, m.y + 8);

  ctx.font = '11px ui-sans-serif, system-ui, sans-serif';
  ctx.fillStyle = COLOR_LABEL_SECONDARY;
  const res = `${m.pixelW}×${m.pixelH}`;
  ctx.fillText(res, m.x + 8, m.y + 24);

  if (m.h >= 60 && m.w >= 110) {
    const phys = m.missing
      ? 'physical size unknown'
      : `${Math.round(m.widthMm)}×${Math.round(m.heightMm)} mm`;
    ctx.fillStyle = m.missing ? COLOR_LABEL_MISSING : COLOR_LABEL_SECONDARY;
    ctx.fillText(phys, m.x + 8, m.y + 40);
  }
}

function drawResizeHandles(ctx: CanvasRenderingContext2D, imgRect: ImageRect): void {
  const half = RESIZE_HANDLE_PX / 2;
  const corners = resizeHandleCorners(imgRect);
  ctx.save();
  ctx.lineWidth = 1.5;
  ctx.strokeStyle = COLOR_RESIZE_HANDLE_BORDER;
  ctx.fillStyle = COLOR_RESIZE_HANDLE;
  for (const c of corners) {
    ctx.fillRect(c.x - half, c.y - half, RESIZE_HANDLE_PX, RESIZE_HANDLE_PX);
    ctx.strokeRect(c.x - half, c.y - half, RESIZE_HANDLE_PX, RESIZE_HANDLE_PX);
  }
  ctx.restore();
}

export type ResizeCorner = 'tl' | 'tr' | 'bl' | 'br';

export function resizeHandleCorners(
  imgRect: ImageRect,
): { x: number; y: number; id: ResizeCorner }[] {
  return [
    { x: imgRect.x, y: imgRect.y, id: 'tl' },
    { x: imgRect.x + imgRect.w, y: imgRect.y, id: 'tr' },
    { x: imgRect.x, y: imgRect.y + imgRect.h, id: 'bl' },
    { x: imgRect.x + imgRect.w, y: imgRect.y + imgRect.h, id: 'br' },
  ];
}

export function hitResizeHandle(imgRect: ImageRect, x: number, y: number): ResizeCorner | null {
  const half = RESIZE_HANDLE_PX / 2;
  for (const c of resizeHandleCorners(imgRect)) {
    if (Math.abs(c.x - x) <= half && Math.abs(c.y - y) <= half) {
      return c.id;
    }
  }
  return null;
}

export function hitTest(layout: CanvasLayout, x: number, y: number): number | null {
  for (let i = layout.monitors.length - 1; i >= 0; i -= 1) {
    const m = layout.monitors[i];
    if (!m) continue;
    if (x >= m.x && x <= m.x + m.w && y >= m.y && y <= m.y + m.h) {
      return m.monitorIndex;
    }
  }
  return null;
}

export function pointInsideRect(rect: ImageRect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h;
}
