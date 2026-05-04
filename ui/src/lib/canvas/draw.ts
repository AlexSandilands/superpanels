// Five-layer canvas compositing per SPEC §12.3:
//   1. Wallpaper image (thumbnail), positioned per the live offset.
//   2. Dark overlay covering the non-monitor canvas area.
//   3. Cut-outs revealing the image through monitor windows.
//   4. Bezel bars in the gaps.
//   5. Outlines + labels.
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
  // Live drag offset applied to the image, in canvas pixels.
  offsetX: number;
  offsetY: number;
  fit: 'fill' | 'fit' | 'stretch' | 'center';
  // Index of the monitor under the cursor (highlight) — `null` when none.
  hoverIndex: number | null;
  // Labels visible at this zoom level. Disable when very zoomed-out to avoid clutter.
  showLabels: boolean;
};

const COLOR_BG = '#0b1220';
const COLOR_OVERLAY = 'rgba(0, 0, 0, 0.55)';
const COLOR_BEZEL = '#0b1220';
const COLOR_OUTLINE = 'rgba(96, 165, 250, 0.85)';
const COLOR_OUTLINE_HOVER = 'rgba(125, 211, 252, 1)';
const COLOR_OUTLINE_MISSING = 'rgba(245, 158, 11, 0.9)';
const COLOR_LABEL = '#e2e8f0';
const COLOR_LABEL_SECONDARY = '#94a3b8';
const COLOR_LABEL_MISSING = '#fbbf24';

export function drawCanvasLayers(
  ctx: CanvasRenderingContext2D,
  layout: CanvasLayout,
  opts: DrawOptions,
): void {
  const { dpr, viewportW, viewportH } = opts;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  // 0. Background.
  ctx.fillStyle = COLOR_BG;
  ctx.fillRect(0, 0, viewportW, viewportH);

  if (layout.monitors.length === 0) {
    ctx.restore();
    return;
  }

  // 1. Wallpaper image positioned over the layout area.
  drawWallpaper(ctx, layout, opts);

  // 2 + 3. Dark overlay with monitor cut-outs. Drawing this as one even-odd
  // path keeps the already-drawn wallpaper intact inside the monitor windows.
  drawOverlayWithCutouts(ctx, layout, viewportW, viewportH);

  // 4. Bezel bars over the image (the gap between monitors physically blocks
  //    the image, so paint dark over those regions).
  ctx.fillStyle = COLOR_BEZEL;
  for (const b of layout.bezels) {
    ctx.fillRect(b.x, b.y, b.w, b.h);
  }

  // 5. Outlines + labels.
  for (const m of layout.monitors) {
    drawMonitorChrome(ctx, m, opts);
  }

  ctx.restore();
}

function drawOverlayWithCutouts(
  ctx: CanvasRenderingContext2D,
  layout: CanvasLayout,
  viewportW: number,
  viewportH: number,
): void {
  ctx.beginPath();
  ctx.rect(0, 0, viewportW, viewportH);
  for (const m of layout.monitors) {
    ctx.rect(m.x, m.y, m.w, m.h);
  }
  ctx.fillStyle = COLOR_OVERLAY;
  ctx.fill('evenodd');
}

function drawWallpaper(
  ctx: CanvasRenderingContext2D,
  layout: CanvasLayout,
  opts: DrawOptions,
): void {
  const { image, imageW, imageH, fit, offsetX, offsetY } = opts;
  const { offsetX: layoutX, offsetY: layoutY, totalW, totalH } = layout;

  // Clip to the layout's bounding box so the image never bleeds into the
  // padding around the canvas.
  ctx.save();
  ctx.beginPath();
  ctx.rect(layoutX, layoutY, totalW, totalH);
  ctx.clip();

  if (!image || imageW <= 0 || imageH <= 0) {
    ctx.fillStyle = '#1e293b';
    ctx.fillRect(layoutX, layoutY, totalW, totalH);
    ctx.restore();
    return;
  }

  const placement = placeImage(fit, totalW, totalH, imageW, imageH);
  ctx.drawImage(
    image,
    layoutX + placement.x + offsetX,
    layoutY + placement.y + offsetY,
    placement.w,
    placement.h,
  );

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
  ctx.lineWidth = isHover ? 2 : 1.25;
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
