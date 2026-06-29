// Cross-store actions used from `App.svelte` — apply, switch profile, and the
// slideshow next/prev/pause triplet. Each is async and turns errors into
// toasts; they don't return error state because callers just `void` them.

import { getCurrentWindow } from '@tauri-apps/api/window';
import { api, errorMessage, type Profile } from '$lib/api';
import { buildPreviewMonitors } from '$lib/canvas/preview-layout';
import { canvasView, type MonitorOverride } from '$lib/stores/canvas-view.svelte';
import { canvasLayers } from '$lib/stores/canvas-layers.svelte';
import { imageTransform } from '$lib/stores/image-transform.svelte';
import { monitorStore } from '$lib/stores/monitors.svelte';
import { preemption } from '$lib/stores/preemption.svelte';
import { profileStore } from '$lib/stores/profile.svelte';
// Cyclic with slideshow-controller (it imports next/prev/refreshRuntime from
// here) — safe: both modules only touch the other inside functions.
import { slideshowController } from '$lib/slideshow-controller.svelte';
import { addImage } from '$lib/slideshow-set';
import { runtime } from '$lib/stores/runtime.svelte';
import { toast } from '$lib/stores/toast.svelte';
import type { ImageRectMm } from '$lib/types/ImageRectMm';
import type { MonitorPlacement } from '$lib/types/MonitorPlacement';
import { isSlideshowBody, isStandardBody, type SlideshowSource } from '$lib/types/profile-helpers';

function imageRectFromTransform(): ImageRectMm {
  const t = imageTransform.value;
  return { x_mm: t.offsetMmX, y_mm: t.offsetMmY, w_mm: t.widthMm, h_mm: t.heightMm };
}

// Fold the current canvas (detected monitors + canvasView overrides + image
// transform) into the active draft so a fresh untitled profile actually
// carries placements when it gets persisted on Apply. Topology is left empty
// here — the daemon recomputes the canonical SHA-256 fingerprint when
// `recompute_topology` is set on save.
function syncDraftFromCanvas(): void {
  if (!profileStore.draft) return;
  const previews = buildPreviewMonitors(monitorStore.monitors, canvasView.overrides);
  if (previews.length === 0) return;
  const nextPlacements: Record<string, MonitorPlacement> = {};
  for (const m of previews) {
    nextPlacements[m.id] = { x_mm: m.xMm, y_mm: m.yMm };
  }
  const rect = imageRectFromTransform();
  profileStore.patchDraft((d) => {
    d.monitor_state = nextPlacements;
    d.topology = '';
    if (isStandardBody(d.body)) {
      d.body.layers = canvasLayers.toLayers();
    } else if (isSlideshowBody(d.body)) {
      d.body.image_rect_mm = rect;
    }
  });
}

// Push a profile's authored placements into `canvasView.overrides` and
// (when the body is a Span) its `image_rect_mm` into the live image
// transform, so the canvas reflects what was just applied. Existing entries
// for monitors not in the profile are preserved.
export function applyMonitorStateToCanvas(p: Profile): void {
  const next: Record<string, MonitorOverride> = { ...canvasView.overrides };
  for (const [id, placement] of Object.entries(p.monitor_state)) {
    next[id] = {
      xMm: placement.x_mm,
      yMm: placement.y_mm,
    };
  }
  canvasView.setOverrides(next);
  if (isStandardBody(p.body)) {
    // Always reseed the layer stack from the profile — this is what stops a
    // previous profile's layers bleeding onto a freshly-selected one.
    canvasLayers.setFromLayers(p.body.layers);
    return;
  }
  // Slideshow rects are owned by the per-image seed (override / uniform /
  // cover-fit, see `useSourceImage`) — forcing the profile rect here would
  // crop the live image to a rect authored for a different aspect. Clear any
  // leftover layers so a switch away from a standard profile doesn't show them.
  canvasLayers.clear();
}

function recordAndToast(r: Awaited<ReturnType<typeof api.applyProfile>>, t0: number): number {
  const elapsed = r.elapsed_ms ?? Math.round(performance.now() - t0);
  runtime.recordApply({
    backend: r.backend ?? 'unknown',
    elapsedMs: elapsed,
    monitorsSet: r.monitors_set ?? monitorStore.monitors.length,
    at: Date.now(),
  });
  return elapsed;
}

/** Push the current canvas state to the desktop without persisting it
 *  (§4e.11.1). The active profile name (if any) is sent alongside so the
 *  daemon updates `last_applied_at` and rotates `active_profile`, but
 *  `monitor_state`, image transform, and source on disk stay untouched. */
export async function applyDraftProfile(): Promise<void> {
  const draft = profileStore.draft;
  if (!draft) return;
  syncDraftFromCanvas();
  const refreshed = profileStore.draft;
  if (!refreshed) return;
  preemption.claimSwitchTo(profileStore.activeName);
  try {
    const t0 = performance.now();
    const r = await api.applyCanvas(refreshed, profileStore.activeName);
    const elapsed = recordAndToast(r, t0);
    toast.success(`Applied '${refreshed.name}'`, `${r.backend ?? 'backend'} · ${elapsed} ms`);
    void profileStore.refresh();
    void slideshowController.refresh();
  } catch (err) {
    preemption.cancelClaim(profileStore.activeName);
    toast.error('Apply failed', errorMessage(err));
  }
}

/** Commit the current canvas state into the active profile's TOML
 *  (§4e.11.3 Save). No-op when there is no active profile. */
export async function saveActiveProfile(): Promise<boolean> {
  const draft = profileStore.draft;
  const active = profileStore.activeName;
  if (!draft || !active) return false;
  syncDraftFromCanvas();
  const refreshed = profileStore.draft;
  if (!refreshed) return false;
  const payload: Profile = { ...refreshed, name: active };
  preemption.claimSwitchTo(active);
  try {
    await api.saveProfile(payload, { recomputeTopology: true });
    void profileStore.refresh();
    profileStore.clearDirty();
    toast.success(`Saved '${active}'`);
    return true;
  } catch (err) {
    preemption.cancelClaim(profileStore.activeName);
    toast.error(`Failed to save '${active}'`, errorMessage(err));
    return false;
  }
}

/** Re-pull the active profile's persisted state into the canvas
 *  (§4e.11.4 Revert). */
export function revertCanvasToActive(): void {
  const saved = profileStore.revertToActive();
  if (!saved) {
    toast.info('Nothing to revert', 'no active profile');
    return;
  }
  applyMonitorStateToCanvas(saved);
  toast.info(`Reverted to '${saved.name}'`);
}

export async function switchAndApply(p: Profile): Promise<void> {
  profileStore.select(p.name);
  applyMonitorStateToCanvas(p);
  preemption.claimSwitchTo(p.name);
  try {
    const t0 = performance.now();
    const r = await api.applyProfile(p.name);
    recordAndToast(r, t0);
    toast.success(`Switched to ${p.name}`);
    void profileStore.refresh();
    // Pull the slideshow's new current image immediately — waiting for the
    // 5 s poll leaves the canvas imageless after switching to a slideshow.
    void slideshowController.refresh();
  } catch (err) {
    preemption.cancelClaim(profileStore.activeName);
    toast.error(`Failed to apply '${p.name}'`, errorMessage(err));
  }
}

export type SlideshowState = {
  paused: boolean;
  /** Position in the resolved pool; `null` when unknown (e.g. after Prev). */
  index: number | null;
  total: number;
  currentPath: string | null;
  /** Seconds until the next automatic advance, as of `fetchedAt`. */
  remainingSecs: number | null;
  fetchedAt: number;
} | null;

export async function refreshRuntime(): Promise<SlideshowState | undefined> {
  try {
    const s = await api.currentState();
    if (s.slideshow) {
      // `?? null` throughout: a daemon missing a field must read as "absent",
      // not leak `undefined` past `=== null` guards downstream.
      return {
        paused: s.slideshow.paused,
        index: s.slideshow.current_index ?? null,
        total: s.slideshow.pool_len ?? s.slideshow.history_len + 1,
        currentPath: s.slideshow.current_path ?? null,
        remainingSecs: s.slideshow.remaining_secs ?? null,
        fetchedAt: Date.now(),
      };
    }
    return null;
  } catch {
    // ignore — `currentState` may not be reachable yet at startup
    return undefined;
  }
}

export async function slideshowNext(): Promise<void> {
  try {
    await api.slideshowNext();
  } catch (err) {
    toast.error('Slideshow next failed', errorMessage(err));
  }
}

export async function slideshowPrev(): Promise<void> {
  try {
    await api.slideshowPrev();
  } catch (err) {
    toast.error('Slideshow prev failed', errorMessage(err));
  }
}

export async function slideshowGoto(path: string): Promise<void> {
  try {
    await api.slideshowGoto(path);
  } catch (err) {
    toast.error('Could not show image', errorMessage(err));
  }
}

/** Add an image as a new layer on the standard canvas. Converts the draft to a
 *  standard body the first time, then appends the layer (cover-fit, on top).
 *  This is the single entry point for putting an image on the canvas — adding
 *  one image or many works the same way. */
export function addImageToCanvas(path: string): void {
  if (!profileStore.draft) profileStore.newProfile();
  profileStore.patchDraft((d) => {
    if (!isStandardBody(d.body)) {
      d.body = { type: 'standard', layers: [] };
    }
  });
  const monitors = buildPreviewMonitors(monitorStore.monitors, canvasView.overrides);
  void canvasLayers.add(path, monitors);
  toast.success('Added to canvas', path.split('/').pop() ?? path);
}

/** Append `path` to the active slideshow's image set, keeping it a slideshow
 *  (the alternative to `addImageToCanvas`'s convert-to-standard). Persists via
 *  `update_profile_source` when there's an active profile, else patches the
 *  unsaved draft in place. */
export async function addImageToSlideshowSet(path: string): Promise<void> {
  const draft = profileStore.draft;
  if (!draft || !isSlideshowBody(draft.body)) return;
  const nextSource: SlideshowSource = {
    ...draft.body.source,
    images: addImage(draft.body.source.images, path),
  };
  const name = profileStore.activeName;
  if (name) {
    const ok = await persistSlideshowSource(name, nextSource);
    if (!ok) return;
  } else {
    profileStore.patchDraft((d) => {
      if (isSlideshowBody(d.body)) d.body.source = nextSource;
    });
  }
  toast.success('Added to slideshow', path.split('/').pop() ?? path);
}

/** Drop an image directly onto a monitor: add it as a standard-canvas layer
 *  contain-fitted to that monitor at the image's aspect — the whole image sits
 *  inside the monitor (letterboxed for wider images, pillarboxed for taller),
 *  never cropped. The snap buttons fill it from there. Falls back to a
 *  whole-desktop cover-fit if the monitor isn't in the current layout. */
export function dropImageOnMonitor(monitorId: string, path: string): void {
  if (!profileStore.draft) profileStore.newProfile();
  profileStore.patchDraft((d) => {
    if (!isStandardBody(d.body)) {
      d.body = { type: 'standard', layers: [] };
    }
  });
  const monitors = buildPreviewMonitors(monitorStore.monitors, canvasView.overrides);
  void canvasLayers.add(path, monitors, monitorId);
  const name = monitors.find((m) => m.id === monitorId)?.name;
  toast.success('Snapped to monitor', `${name ? `${name}: ` : ''}${path.split('/').pop() ?? path}`);
}

// Source writes queue behind one another so a burst of library toggles can't
// land on the daemon out of order.
let pendingSourceWrite: Promise<unknown> = Promise.resolve();

/** Persist a slideshow profile's source (image set and/or timer config) and
 *  mirror it into the in-memory profile list + draft without touching the
 *  dirty flag. The daemon re-tunes the live picker and timer when the target
 *  is the active profile. */
export async function persistSlideshowSource(
  profileName: string,
  source: SlideshowSource,
): Promise<boolean> {
  // Commit to the store before the IPC round-trip so a follow-up toggle
  // computes its next set from this one instead of a stale base.
  profileStore.commitSource(profileName, source);
  const write = pendingSourceWrite.then(() => api.updateProfileSource(profileName, source));
  pendingSourceWrite = write.catch(() => undefined);
  try {
    await write;
    return true;
  } catch (err) {
    toast.error('Slideshow update failed', errorMessage(err));
    // The optimistic commit no longer matches disk — re-pull.
    void profileStore.refresh();
    return false;
  }
}

export async function slideshowTogglePause(): Promise<{ paused: boolean } | null> {
  try {
    return await api.slideshowPause();
  } catch (err) {
    toast.error('Slideshow pause failed', errorMessage(err));
    return null;
  }
}

export async function openMainWindow(): Promise<void> {
  try {
    const w = getCurrentWindow();
    await w.show();
    await w.unminimize();
    await w.setFocus();
  } catch {
    // ignore
  }
}

export async function quitApp(): Promise<void> {
  try {
    await getCurrentWindow().destroy();
  } catch {
    // ignore
  }
}

export async function redetectMonitorsWithToast(): Promise<void> {
  await monitorStore.refresh();
  toast.info(`Re-detected ${monitorStore.monitors.length} monitors`);
}
