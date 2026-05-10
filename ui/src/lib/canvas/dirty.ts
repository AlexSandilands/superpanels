// Pure dirty-diff between the live canvas overrides and the active profile's
// persisted `monitor_state` (§4e.11.3 dirty detection). Image transform
// tracking is deferred until we have a mm↔px converter; see followups.

import type { MonitorOverride } from '$lib/stores/canvas-view.svelte';
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
