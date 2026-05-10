// Cross-store actions used from `App.svelte` — apply, switch profile, and the
// slideshow next/prev/pause triplet. Each is async and turns errors into
// toasts; they don't return error state because callers just `void` them.

import { getCurrentWindow } from '@tauri-apps/api/window';
import { api, errorMessage, type Profile } from '$lib/api';
import { buildPreviewMonitors, stableId } from '$lib/canvas/preview-layout';
import { canvasView, type MonitorOverride } from '$lib/stores/canvas-view.svelte';
import { imageTransform } from '$lib/stores/image-transform.svelte';
import { monitorStore } from '$lib/stores/monitors.svelte';
import { preemption } from '$lib/stores/preemption.svelte';
import { profileStore } from '$lib/stores/profile.svelte';
import { runtime } from '$lib/stores/runtime.svelte';
import { toast } from '$lib/stores/toast.svelte';
import type { ImageRectMm } from '$lib/types/ImageRectMm';
import type { MonitorPlacement } from '$lib/types/MonitorPlacement';
import type { Rotation } from '$lib/types/Rotation';
import {
  isPerMonitorBody,
  isSpanBody,
  type PerMonitorAssignment,
} from '$lib/types/profile-helpers';

function rotationFromDeg(d: 0 | 90 | 180 | 270): Rotation {
  switch (d) {
    case 90:
      return 'right';
    case 180:
      return 'inverted';
    case 270:
      return 'left';
    default:
      return 'none';
  }
}

function rotationToDeg(r: Rotation): 0 | 90 | 180 | 270 {
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
    nextPlacements[m.id] = { x_mm: m.xMm, y_mm: m.yMm, rotation: rotationFromDeg(m.rotation) };
  }
  const rect = imageRectFromTransform();
  profileStore.patchDraft((d) => {
    d.monitor_state = nextPlacements;
    d.topology = '';
    if (isSpanBody(d.body)) {
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
      rotation: rotationToDeg(placement.rotation),
    };
  }
  canvasView.setOverrides(next);
  if (isSpanBody(p.body)) {
    const r = p.body.image_rect_mm;
    imageTransform.set({
      offsetMmX: r.x_mm,
      offsetMmY: r.y_mm,
      widthMm: r.w_mm,
      heightMm: r.h_mm,
    });
  }
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
  } catch (err) {
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
  } catch (err) {
    toast.error(`Failed to apply '${p.name}'`, errorMessage(err));
  }
}

export type SlideshowState = { paused: boolean; index: number; total: number } | null;

export async function refreshRuntime(): Promise<SlideshowState | undefined> {
  try {
    const s = await api.currentState();
    if (s.slideshow) {
      return {
        paused: s.slideshow.paused,
        index: s.slideshow.current_index ?? 0,
        total: s.slideshow.history_len + 1,
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

export function setSpanImage(path: string): void {
  if (!profileStore.draft) profileStore.newProfile();
  profileStore.patchDraft((d) => {
    if (!isSpanBody(d.body)) {
      d.body = {
        type: 'span',
        source: { type: 'single', path },
        image_rect_mm: imageRectFromTransform(),
      };
      return;
    }
    if (d.body.source.type === 'single') {
      d.body.source.path = path;
    } else {
      d.body.source = { type: 'single', path };
    }
  });
  toast.success('Source updated', path.split('/').pop() ?? path);
}

export function pinImageToMonitor(monitorId: string, path: string): void {
  if (!profileStore.draft) profileStore.newProfile();
  const detected = monitorStore.monitors.find((m) => stableId(m) === monitorId);
  if (!detected) {
    toast.error('Drop ignored', 'monitor not found in layout');
    return;
  }
  const assignment: PerMonitorAssignment = {
    monitor: { stable_id: detected.stable_id ?? '', name: detected.name },
    path,
  };
  profileStore.patchDraft((d) => {
    if (!isPerMonitorBody(d.body)) {
      d.body = { type: 'per_monitor', assignments: [assignment], fit: 'fill' };
      return;
    }
    const idx = d.body.assignments.findIndex(
      (a) =>
        (a.monitor.stable_id !== '' && a.monitor.stable_id === assignment.monitor.stable_id) ||
        a.monitor.name === assignment.monitor.name,
    );
    if (idx >= 0) d.body.assignments[idx] = assignment;
    else d.body.assignments.push(assignment);
  });
  toast.success('Image pinned', `${detected.name}: ${path.split('/').pop() ?? path}`);
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
