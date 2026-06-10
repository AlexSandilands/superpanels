// Free-positioning image transform in mm-space, plus the override-seeding
// effect that keeps `canvasView.overrides` in sync with the detected monitor
// list (preserving any user edits on incremental detection updates).

import { untrack } from 'svelte';
import { errorMessage, type Monitor } from '$lib/api';
import { canvasView, type MonitorOverride } from '$lib/stores/canvas-view.svelte';
import { coverImageRect, defaultOverrides, type PreviewMonitor } from '$lib/canvas/preview-layout';
import { loadSourceImage, peekSourceImage } from '$lib/library/source-image';
import { toast } from '$lib/stores/toast.svelte';
import type { MonitorPlacement } from '$lib/types/MonitorPlacement';

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
  /** The path `url`/`naturalDims` belong to — consumers comparing against
   *  the *current* source must check this, since loads resolve async. */
  path: string | null;
};

let sourceState = $state<SourceImageState>({ url: null, naturalDims: null, path: null });

export const sourceImageState = {
  get value() {
    return sourceState;
  },
};

// Resolve the active source image and seed `imageTransform` the first time we
// see a given path: from `getBaselineTransform` when the caller has an
// authored rect for it (slideshow per-image override or profile-level rect),
// else to a cover-fit rect. Returns the effect cleanup so the component can
// hook it into its own lifecycle.
export function useSourceImage(
  getSourcePath: () => string | null,
  getPreviewMonitors: () => PreviewMonitor[],
  getBaselineTransform?: (path: string) => ImageTransform | null,
): void {
  let initializedFor = '';
  $effect(() => {
    const path = getSourcePath();
    if (!path) {
      sourceState = { url: null, naturalDims: null, path: null };
      initializedFor = '';
      return;
    }
    const cached = peekSourceImage(path);
    if (cached) {
      sourceState = {
        url: cached.url,
        naturalDims: { w: cached.naturalW, h: cached.naturalH },
        path,
      };
      maybeInitTransform(path, getPreviewMonitors());
      return;
    }
    let cancelled = false;
    void loadSourceImage(path)
      .then((img) => {
        if (cancelled || getSourcePath() !== path) return;
        sourceState = { url: img.url, naturalDims: { w: img.naturalW, h: img.naturalH }, path };
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
    const baseline = getBaselineTransform?.(path) ?? null;
    if (baseline) {
      transform = baseline;
      initializedFor = path;
      return;
    }
    if (!sourceState.naturalDims || monitors.length === 0) return;
    const aspect = sourceState.naturalDims.w / sourceState.naturalDims.h;
    transform = coverImageRect(monitors, aspect);
    initializedFor = path;
  }
}

// Re-place monitors when the live slideshow image changes, so the canvas
// follows per-image overrides live: the new image's override placements when
// one was authored, else the profile's authored state. The image rect is
// seeded by `useSourceImage` once the image resolves. Fires only on a path
// change — user drags between advances stay untouched.
export function followSlideshowLayout(
  getLivePath: () => string | null,
  getPlacements: () => Record<string, MonitorPlacement> | null,
): void {
  let lastPath: string | null = null;
  $effect(() => {
    const path = getLivePath();
    const placements = getPlacements();
    if (path === lastPath) return;
    lastPath = path;
    if (!path || !placements) return;
    untrack(() => {
      const next = { ...canvasView.overrides };
      for (const [id, p] of Object.entries(placements)) {
        next[id] = { xMm: p.x_mm, yMm: p.y_mm };
      }
      canvasView.setOverrides(next);
    });
  });
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
