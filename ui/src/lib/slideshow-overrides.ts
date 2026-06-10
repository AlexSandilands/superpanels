// Per-image slideshow override actions: persist through the shared
// `update_profile_source` pipeline, then transiently apply the tweaked
// layout so the desktop reflects it without advancing the slideshow.

import { api, errorMessage, type Profile } from '$lib/api';
import { applyMonitorStateToCanvas, persistSlideshowSource } from '$lib/actions';
import { preemption } from '$lib/stores/preemption.svelte';
import { profileStore } from '$lib/stores/profile.svelte';
import { toast } from '$lib/stores/toast.svelte';
import type { ImageRectMm } from '$lib/types/ImageRectMm';
import type { MonitorPlacement } from '$lib/types/MonitorPlacement';
import { isSpanBody, type SlideshowSource } from '$lib/types/profile-helpers';

export type CanvasLayout = {
  monitor_state: Record<string, MonitorPlacement>;
  image_rect_mm: ImageRectMm;
};

// Push one image with an explicit layout to the desktop without advancing
// the slideshow: a transient single-source span rides `apply_canvas`, so
// nothing is persisted and the picker's position is untouched.
async function applyTransientImageLayout(
  draft: Profile,
  path: string,
  layout: CanvasLayout,
): Promise<void> {
  const transient: Profile = {
    ...draft,
    body: {
      type: 'span',
      source: { type: 'single', path },
      image_rect_mm: layout.image_rect_mm,
    },
    monitor_state: layout.monitor_state,
  };
  preemption.claimSwitchTo(profileStore.activeName);
  try {
    await api.applyCanvas(transient, profileStore.activeName);
  } catch (err) {
    toast.error('Apply failed', errorMessage(err));
  }
}

/** Persist `layout` as `path`'s per-image override on the slideshow profile
 *  `name`, then apply it so the desktop shows the tweak immediately. */
export async function saveOverrideForImage(
  name: string,
  source: SlideshowSource,
  draft: Profile,
  path: string,
  layout: CanvasLayout,
): Promise<void> {
  const overrides = { ...(source.overrides ?? {}), [path]: layout };
  const ok = await persistSlideshowSource(name, { ...source, overrides });
  if (!ok) return;
  toast.success('Saved layout for this image', path.split('/').pop() ?? path);
  await applyTransientImageLayout(draft, path, layout);
}

/** Drop the per-image override for `path`. When that image is on screen
 *  (`livePath`), the canvas and desktop snap back to the profile layout. */
export async function removeOverrideForImage(
  name: string,
  source: SlideshowSource,
  draft: Profile,
  path: string,
  livePath: string | null,
): Promise<void> {
  const existing = source.overrides ?? {};
  if (!(path in existing)) return;
  const overrides = Object.fromEntries(Object.entries(existing).filter(([p]) => p !== path));
  const ok = await persistSlideshowSource(name, { ...source, overrides });
  if (!ok) return;
  toast.success('Removed custom layout', path.split('/').pop() ?? path);
  if (path !== livePath) return;
  const active = profileStore.activeProfile;
  if (!active) return;
  applyMonitorStateToCanvas(active);
  if (isSpanBody(active.body)) {
    await applyTransientImageLayout(draft, path, {
      monitor_state: active.monitor_state,
      image_rect_mm: active.body.image_rect_mm,
    });
  }
}
