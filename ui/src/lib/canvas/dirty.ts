// Pure dirty-diff between the live canvas state (monitor overrides + image
// transform) and the active profile's persisted state.

import type { MonitorOverride } from '$lib/stores/canvas-view.svelte';
import type { ImageTransform } from '$lib/stores/image-transform.svelte';
import type { Profile } from '$lib/api';

const POSITION_TOLERANCE_MM = 0.5;

function rotationDegrees(r: 'none' | 'right' | 'inverted' | 'left'): 0 | 90 | 180 | 270 {
  switch (r) {
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

/** Returns `true` when any live monitor override differs (beyond the
 *  half-millimetre slop tolerance) from the persisted placement. */
export function canvasOverridesDirty(
  overrides: Record<string, MonitorOverride>,
  profile: Profile,
): boolean {
  for (const [id, persisted] of Object.entries(profile.monitor_state)) {
    const live = overrides[id];
    if (!live) continue;
    if (
      Math.abs(live.xMm - persisted.x_mm) > POSITION_TOLERANCE_MM ||
      Math.abs(live.yMm - persisted.y_mm) > POSITION_TOLERANCE_MM
    ) {
      return true;
    }
    if (live.rotation !== rotationDegrees(persisted.rotation)) return true;
  }
  return false;
}

/** Returns `true` when the live image transform differs (beyond the slop
 *  tolerance) from the active span profile's `image_rect_mm`. Per-monitor
 *  bodies have no image rect — they're always clean from this lens. */
export function imageTransformDirty(transform: ImageTransform, profile: Profile): boolean {
  if (profile.body.type !== 'span') return false;
  const r = profile.body.image_rect_mm;
  return (
    Math.abs(transform.offsetMmX - r.x_mm) > POSITION_TOLERANCE_MM ||
    Math.abs(transform.offsetMmY - r.y_mm) > POSITION_TOLERANCE_MM ||
    Math.abs(transform.widthMm - r.w_mm) > POSITION_TOLERANCE_MM ||
    Math.abs(transform.heightMm - r.h_mm) > POSITION_TOLERANCE_MM
  );
}
