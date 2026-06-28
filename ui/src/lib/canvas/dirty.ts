// Pure dirty-diff between the live canvas state (monitor overrides + image
// transform) and a persisted baseline. The baseline is normally the active
// profile; for a slideshow image with a per-image override it is that
// override instead — callers pick via `placementsDirty` / `rectDirty`.

import type { MonitorOverride } from '$lib/stores/canvas-view.svelte';
import type { CanvasLayer } from '$lib/stores/canvas-layers.svelte';
import type { ImageTransform } from '$lib/stores/image-transform.svelte';
import type { Profile } from '$lib/api';
import type { ImageRectMm } from '$lib/types/ImageRectMm';
import type { MonitorPlacement } from '$lib/types/MonitorPlacement';
import type { StandardLayer } from '$lib/types/profile-helpers';
import { coverImageRect, type PreviewMonitor } from './preview-layout';

const POSITION_TOLERANCE_MM = 0.5;

/** Returns `true` when any live monitor override differs (beyond the
 *  half-millimetre slop tolerance) from the persisted placement. */
export function placementsDirty(
  overrides: Record<string, MonitorOverride>,
  persisted: Record<string, MonitorPlacement>,
): boolean {
  for (const [id, placement] of Object.entries(persisted)) {
    const live = overrides[id];
    if (!live) continue;
    if (
      Math.abs(live.xMm - placement.x_mm) > POSITION_TOLERANCE_MM ||
      Math.abs(live.yMm - placement.y_mm) > POSITION_TOLERANCE_MM
    ) {
      return true;
    }
  }
  return false;
}

/** Returns `true` when the live image transform differs (beyond the slop
 *  tolerance) from the persisted rect. */
export function rectDirty(transform: ImageTransform, r: ImageRectMm): boolean {
  return (
    Math.abs(transform.offsetMmX - r.x_mm) > POSITION_TOLERANCE_MM ||
    Math.abs(transform.offsetMmY - r.y_mm) > POSITION_TOLERANCE_MM ||
    Math.abs(transform.widthMm - r.w_mm) > POSITION_TOLERANCE_MM ||
    Math.abs(transform.heightMm - r.h_mm) > POSITION_TOLERANCE_MM
  );
}

/** [`placementsDirty`] against the profile's authored `monitor_state`. */
export function canvasOverridesDirty(
  overrides: Record<string, MonitorOverride>,
  profile: Profile,
): boolean {
  return placementsDirty(overrides, profile.monitor_state);
}

/** [`rectDirty`] against the active slideshow profile's `image_rect_mm`.
 *  Standard / per-monitor bodies have no profile-level image rect — they're
 *  always clean here. */
export function imageTransformDirty(transform: ImageTransform, profile: Profile): boolean {
  if (profile.body.type !== 'slideshow') return false;
  return rectDirty(transform, profile.body.image_rect_mm);
}

/** Returns `true` when the live canvas layers differ from the persisted ones —
 *  count, order, paths, or any rect beyond the slop tolerance. Diffs the live
 *  `CanvasLayer[]` directly so the hot path (re-evaluated every drag frame)
 *  doesn't materialise an intermediate `StandardLayer[]` per frame. */
export function liveLayersDirty(live: CanvasLayer[], persisted: StandardLayer[]): boolean {
  if (live.length !== persisted.length) return true;
  return live.some((l, i) => {
    const p = persisted[i];
    if (!p || p.path !== l.path) return true;
    return rectDirty(l.transform, p.image_rect_mm);
  });
}

/** [`rectDirty`] against the cover-fit rect for `naturalDims` over
 *  `monitors` — the clean baseline for a slideshow image without a per-image
 *  override (the canvas seeds it and the daemon applies it). Unknown dims
 *  read as clean: there is nothing to compare yet. */
export function coverRectDirty(
  transform: ImageTransform,
  monitors: PreviewMonitor[],
  naturalDims: { w: number; h: number } | null,
): boolean {
  if (!naturalDims || monitors.length === 0) return false;
  const c = coverImageRect(monitors, naturalDims.w / naturalDims.h);
  return rectDirty(transform, {
    x_mm: c.offsetMmX,
    y_mm: c.offsetMmY,
    w_mm: c.widthMm,
    h_mm: c.heightMm,
  });
}
