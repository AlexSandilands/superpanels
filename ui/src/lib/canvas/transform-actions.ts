// Whole-canvas mutations: gap normalisation, image-cover snap, transform/
// layout reset. Each is a small orchestration that touches more than one
// store, so it lives here rather than in any single store module.

import {
  coverImageRect,
  defaultOverrides,
  hNeighbourPairs,
  normaliseHGaps,
  normaliseVGaps,
  vNeighbourPairs,
  type PreviewMonitor,
} from '$lib/canvas/preview-layout';
import { cover, fitHeight, fitWidth, targetRectForLayer } from '$lib/canvas/snap';
import { canvasLayers } from '$lib/stores/canvas-layers.svelte';
import { canvasView } from '$lib/stores/canvas-view.svelte';
import { imageTransform } from '$lib/stores/image-transform.svelte';
import { monitorStore } from '$lib/stores/monitors.svelte';
import { profileStore } from '$lib/stores/profile.svelte';
import { toast } from '$lib/stores/toast.svelte';

export type SnapKind = 'width' | 'height' | 'cover';

export function setHGap(monitors: PreviewMonitor[], _vMm: number, hMm: number): void {
  // Bezel field is gone; this just normalises the canvas placements so the
  // gap between adjacent monitors becomes `hMm`. The change is captured into
  // the active profile via the canvas auto-save path (4e.3).
  if (hNeighbourPairs(monitors).length === 0) return;
  canvasView.setOverrides(normaliseHGaps(monitors, canvasView.overrides, hMm));
  // Suppress the unused-import warning until 4e.3 wires up auto-save here.
  void profileStore;
}

export function setVGap(monitors: PreviewMonitor[], _hMm: number, vMm: number): void {
  if (vNeighbourPairs(monitors).length === 0) return;
  canvasView.setOverrides(normaliseVGaps(monitors, canvasView.overrides, vMm));
  void profileStore;
}

export function resetLayout(bezelHmm: number): void {
  canvasView.resetOverrides(defaultOverrides(monitorStore.monitors, bezelHmm));
  toast.info('Monitor layout reset');
}

export function snapCover(
  monitors: PreviewMonitor[],
  naturalDims: { w: number; h: number } | null,
): void {
  if (!naturalDims) {
    toast.info('No image loaded', 'pick one from the library first');
    return;
  }
  const next = coverImageRect(monitors, naturalDims.w / naturalDims.h);
  imageTransform.set(next);
  toast.info(
    'Snapped image to cover',
    `${Math.round(next.widthMm)}×${Math.round(next.heightMm)} mm`,
  );
}

export function resetTransform(
  monitors: PreviewMonitor[],
  naturalDims: { w: number; h: number } | null,
): void {
  if (!naturalDims) return;
  imageTransform.set(coverImageRect(monitors, naturalDims.w / naturalDims.h));
  toast.info('Image transform reset');
}

/** Snap one layer onto the monitor(s) it overlaps, preserving aspect.
 *  `width`/`height` are the single-axis fits; `cover` fills (no letterbox). */
export function snapLayer(id: string, monitors: PreviewMonitor[], kind: SnapKind): void {
  const layer = canvasLayers.list.find((l) => l.id === id);
  if (!layer) return;
  const target = targetRectForLayer(layer.transform, monitors);
  if (!target) {
    toast.info('Not over a monitor', 'drag the image onto a monitor, then snap');
    return;
  }
  const aspect = layer.transform.widthMm / layer.transform.heightMm;
  const next =
    kind === 'width'
      ? fitWidth(target, aspect)
      : kind === 'height'
        ? fitHeight(target, aspect)
        : cover(target, aspect);
  canvasLayers.patch(id, next);
}

/** Snap the currently-selected layer (toolbar / keyboard entry point). */
export function snapSelectedLayer(monitors: PreviewMonitor[], kind: SnapKind): void {
  const id = canvasView.selectedLayerId;
  if (!id) {
    toast.info('No image selected', 'click an image on the canvas first');
    return;
  }
  snapLayer(id, monitors, kind);
}

/** Reset the selected layer to cover the whole desktop (all monitors) — the
 *  cover-fit seed a freshly-added layer starts from. */
export function resetSelectedLayer(monitors: PreviewMonitor[]): void {
  const id = canvasView.selectedLayerId;
  const layer = id ? canvasLayers.list.find((l) => l.id === id) : null;
  if (!layer || !id) {
    toast.info('No image selected', 'click an image on the canvas first');
    return;
  }
  const aspect = layer.transform.widthMm / layer.transform.heightMm;
  canvasLayers.patch(id, coverImageRect(monitors, aspect));
  toast.info('Reset image to cover');
}
