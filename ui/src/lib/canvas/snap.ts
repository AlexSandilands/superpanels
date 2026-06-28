// Snapping a free-positioned layer onto the monitor(s) it sits over. All maths
// is in canvas mm-space (the same plane the layer rect and monitors share).
// Aspect ratio is always preserved; the layer recenters on the target.

import type { ImageTransform } from '$lib/stores/image-transform.svelte';
import type { PreviewMonitor } from './preview-layout';

export type Rect = { x: number; y: number; w: number; h: number };

/** A monitor counts as a snap target once the layer covers at least this much
 *  of its area — so "drag roughly over the two monitors I want, then snap"
 *  lands predictably without a stray sliver pulling in a third. */
const COVERAGE_THRESHOLD = 0.5;

function overlapArea(a: Rect, b: Rect): number {
  const w = Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x));
  const h = Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));
  return w * h;
}

function transformToRect(t: ImageTransform): Rect {
  return { x: t.offsetMmX, y: t.offsetMmY, w: t.widthMm, h: t.heightMm };
}

function monitorRect(m: PreviewMonitor): Rect {
  return { x: m.xMm, y: m.yMm, w: m.wMm, h: m.hMm };
}

/** The rectangle the layer should snap to: the bounding box of every monitor it
 *  covers by at least [`COVERAGE_THRESHOLD`], or — when none reach that — the
 *  single monitor it overlaps most. `null` when the layer overlaps nothing. */
export function targetRectForLayer(layer: ImageTransform, monitors: PreviewMonitor[]): Rect | null {
  if (monitors.length === 0) return null;
  const lr = transformToRect(layer);

  const targeted = monitors.filter((m) => {
    const mr = monitorRect(m);
    const area = mr.w * mr.h;
    return area > 0 && overlapArea(lr, mr) / area >= COVERAGE_THRESHOLD;
  });

  const chosen =
    targeted.length > 0
      ? targeted
      : (() => {
          let best: PreviewMonitor | null = null;
          let bestArea = 0;
          for (const m of monitors) {
            const a = overlapArea(lr, monitorRect(m));
            if (a > bestArea) {
              bestArea = a;
              best = m;
            }
          }
          return best && bestArea > 0 ? [best] : [];
        })();

  if (chosen.length === 0) return null;
  const rects = chosen.map(monitorRect);
  const x0 = Math.min(...rects.map((r) => r.x));
  const y0 = Math.min(...rects.map((r) => r.y));
  const x1 = Math.max(...rects.map((r) => r.x + r.w));
  const y1 = Math.max(...rects.map((r) => r.y + r.h));
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

function centeredOn(target: Rect, w: number, h: number): ImageTransform {
  return {
    offsetMmX: target.x + (target.w - w) / 2,
    offsetMmY: target.y + (target.h - h) / 2,
    widthMm: w,
    heightMm: h,
  };
}

/** Fit the layer to the target's width (height follows aspect): the image sits
 *  within the left/right edges, letterboxing top/bottom if it's wider-fitting. */
export function fitWidth(target: Rect, aspect: number): ImageTransform {
  const w = target.w;
  return centeredOn(target, w, w / aspect);
}

/** Fit the layer to the target's height (width follows aspect): the image fills
 *  top to bottom, overflowing (and cropping) left/right if it's wider. */
export function fitHeight(target: Rect, aspect: number): ImageTransform {
  const h = target.h;
  return centeredOn(target, h * aspect, h);
}

/** Cover the target fully (no letterbox), preserving aspect — the larger of the
 *  two single-axis fits. The repurposed "snap to cover". */
export function cover(target: Rect, aspect: number): ImageTransform {
  const byWidth = fitWidth(target, aspect);
  // Width-fit covers vertically only when its height already spans the target.
  return byWidth.heightMm >= target.h ? byWidth : fitHeight(target, aspect);
}
