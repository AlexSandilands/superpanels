// Free-positioning image transform in mm-space, plus the override-seeding
// effect that keeps `canvasView.overrides` in sync with the detected monitor
// list (preserving any user edits on incremental detection updates).

import { untrack } from 'svelte';
import { errorMessage, type Monitor } from '$lib/api';
import { canvasView, type MonitorOverride } from '$lib/stores/canvasView.svelte';
import { coverImageRect, defaultOverrides, type PreviewMonitor } from '$lib/canvas/previewLayout';
import { loadSourceImage, peekSourceImage } from '$lib/canvas/sourceImage';
import { toast } from '$lib/stores/toast.svelte';

export type ImageTransform = {
  offsetMmX: number;
  offsetMmY: number;
  widthMm: number;
  heightMm: number;
};

let transform = $state<ImageTransform>({
  offsetMmX: 0,
  offsetMmY: 0,
  widthMm: 1800,
  heightMm: 506.25,
});

export const imageTransform = {
  get value() {
    return transform;
  },
  set(next: ImageTransform) {
    transform = next;
  },
  patch(patch: Partial<ImageTransform>) {
    transform = { ...transform, ...patch };
  },
};

export type SourceImageState = {
  url: string | null;
  naturalDims: { w: number; h: number } | null;
};

let sourceState = $state<SourceImageState>({ url: null, naturalDims: null });

export const sourceImageState = {
  get value() {
    return sourceState;
  },
};

// Resolve the active source image and seed `imageTransform` to a cover-fit
// rect the first time we see a given path. Returns the effect cleanup so the
// component can hook it into its own lifecycle.
export function useSourceImage(
  getSourcePath: () => string | null,
  getPreviewMonitors: () => PreviewMonitor[],
): void {
  let initializedFor = '';
  $effect(() => {
    const path = getSourcePath();
    if (!path) {
      sourceState = { url: null, naturalDims: null };
      initializedFor = '';
      return;
    }
    const cached = peekSourceImage(path);
    if (cached) {
      sourceState = { url: cached.url, naturalDims: { w: cached.naturalW, h: cached.naturalH } };
      maybeInitTransform(path, getPreviewMonitors());
      return;
    }
    let cancelled = false;
    void loadSourceImage(path)
      .then((img) => {
        if (cancelled || getSourcePath() !== path) return;
        sourceState = { url: img.url, naturalDims: { w: img.naturalW, h: img.naturalH } };
        maybeInitTransform(path, getPreviewMonitors());
      })
      .catch((err: unknown) => {
        if (cancelled || getSourcePath() !== path) return;
        toast.error('Could not load image', errorMessage(err));
      });
    return () => {
      cancelled = true;
    };
  });

  function maybeInitTransform(path: string, monitors: PreviewMonitor[]): void {
    if (initializedFor === path) return;
    if (!sourceState.naturalDims || monitors.length === 0) return;
    const aspect = sourceState.naturalDims.w / sourceState.naturalDims.h;
    transform = coverImageRect(monitors, aspect);
    initializedFor = path;
  }
}

// Keep `canvasView.overrides` aligned with the detected monitor list. Reads
// `monitors` + `bezelHmm` reactively; writes overrides via untrack so the
// effect doesn't re-fire on its own update. User overrides (existing entries)
// are preserved; only new ids get the layout default.
export function seedOverridesFromMonitors(
  getMonitors: () => Monitor[],
  getBezelHmm: () => number,
): void {
  $effect(() => {
    const detected = getMonitors();
    const hMm = getBezelHmm();
    if (detected.length === 0) return;
    untrack(() => {
      const defaults = defaultOverrides(detected, hMm);
      const current = canvasView.overrides;
      const next: Record<string, MonitorOverride> = { ...defaults };
      for (const id of Object.keys(defaults)) {
        const ex = current[id];
        if (ex) next[id] = ex;
      }
      canvasView.setOverrides(next);
    });
  });
}
