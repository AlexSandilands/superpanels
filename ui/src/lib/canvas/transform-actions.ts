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
import { canvasView } from '$lib/stores/canvas-view.svelte';
import { imageTransform } from '$lib/stores/image-transform.svelte';
import { monitorStore } from '$lib/stores/monitors.svelte';
import { profileStore } from '$lib/stores/profile.svelte';
import { toast } from '$lib/stores/toast.svelte';

export function setHGap(monitors: PreviewMonitor[], vMm: number, hMm: number): void {
  profileStore.patchDraft((d) => {
    d.bezels = { horizontal_mm: hMm, vertical_mm: vMm };
  });
  if (hNeighbourPairs(monitors).length === 0) return;
  canvasView.setOverrides(normaliseHGaps(monitors, canvasView.overrides, hMm));
}

export function setVGap(monitors: PreviewMonitor[], hMm: number, vMm: number): void {
  profileStore.patchDraft((d) => {
    d.bezels = { horizontal_mm: hMm, vertical_mm: vMm };
  });
  if (vNeighbourPairs(monitors).length === 0) return;
  canvasView.setOverrides(normaliseVGaps(monitors, canvasView.overrides, vMm));
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
