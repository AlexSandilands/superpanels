// Composite-mode canvas state: an ordered list of free-positioned images
// (layers) in mm-space, index 0 = bottom. The single-image span path uses
// `image-transform.svelte`; this is its multi-image sibling. Each layer carries
// its own loaded `url` / `naturalDims` (resolved async) and `transform`.

import { errorMessage } from '$lib/api';
import { contain } from '$lib/canvas/snap';
import { coverImageRect, monitorRect, type PreviewMonitor } from '$lib/canvas/preview-layout';
import { CANVAS_MAX_EDGE, loadSourceImage, peekSourceImage } from '$lib/library/source-image';
import { toast } from '$lib/stores/toast.svelte';
import type { ImageTransform } from '$lib/stores/image-transform.svelte';
import type { StandardLayer } from '$lib/types/profile-helpers';

export type CanvasLayer = {
  id: string;
  path: string;
  url: string | null;
  naturalDims: { w: number; h: number } | null;
  transform: ImageTransform;
};

// Each freshly added layer is nudged so it doesn't perfectly eclipse the one
// below — the user drags it into place from there.
const STAGGER_MM = 40;

let layers = $state<CanvasLayer[]>([]);
let counter = 0;

function genId(): string {
  counter += 1;
  return `layer-${counter}`;
}

function rectToTransform(r: StandardLayer['image_rect_mm']): ImageTransform {
  return { offsetMmX: r.x_mm, offsetMmY: r.y_mm, widthMm: r.w_mm, heightMm: r.h_mm };
}

function coverTransform(
  monitors: PreviewMonitor[],
  aspect: number,
  staggerMm: number,
): ImageTransform {
  const r = coverImageRect(monitors, aspect);
  return {
    offsetMmX: r.offsetMmX + staggerMm,
    offsetMmY: r.offsetMmY + staggerMm,
    widthMm: r.widthMm,
    heightMm: r.heightMm,
  };
}

export const canvasLayers = {
  get list(): CanvasLayer[] {
    return layers;
  },

  /** Append a new image on top of the stack. With no `targetMonitorId` the
   *  layer cover-fits over all `monitors`; with one it contain-fits (the whole
   *  image inside that monitor, letterboxed, never cropped) just that monitor —
   *  the drop-onto-a-monitor gesture. The snap buttons fill from there. */
  async add(path: string, monitors: PreviewMonitor[], targetMonitorId?: string): Promise<void> {
    const id = genId();
    const stagger = layers.length * STAGGER_MM;
    const target = targetMonitorId ? monitors.find((m) => m.id === targetMonitorId) : undefined;
    // A monitor-targeted drop is centred on that monitor, so it skips the
    // stagger nudge the all-monitors cover-fit uses to avoid eclipsing siblings.
    const seed = (aspect: number): ImageTransform =>
      target ? contain(monitorRect(target), aspect) : coverTransform(monitors, aspect, stagger);
    const cached = peekSourceImage(path, CANVAS_MAX_EDGE);
    const layer: CanvasLayer = {
      id,
      path,
      url: cached?.url ?? null,
      naturalDims: cached ? { w: cached.naturalW, h: cached.naturalH } : null,
      transform: seed(cached ? cached.naturalW / cached.naturalH : 16 / 9),
    };
    layers = [...layers, layer];
    if (cached) return;
    try {
      const img = await loadSourceImage(path, CANVAS_MAX_EDGE);
      // Re-seed at the true aspect now that we know it (the layer was invisible
      // until now — no url — so the user can't have dragged it yet).
      layers = layers.map((l) =>
        l.id === id
          ? {
              ...l,
              url: img.url,
              naturalDims: { w: img.naturalW, h: img.naturalH },
              transform: seed(img.naturalW / img.naturalH),
            }
          : l,
      );
    } catch (err) {
      layers = layers.filter((l) => l.id !== id);
      toast.error('Could not load image', errorMessage(err));
    }
  },

  remove(id: string): void {
    layers = layers.filter((l) => l.id !== id);
  },

  patch(id: string, transform: ImageTransform): void {
    layers = layers.map((l) => (l.id === id ? { ...l, transform } : l));
  },

  /** Raise `id` to the top of the stack (drag/select brings it to front). */
  bringToFront(id: string): void {
    const idx = layers.findIndex((l) => l.id === id);
    if (idx < 0 || idx === layers.length - 1) return;
    const layer = layers[idx];
    if (!layer) return;
    layers = [...layers.slice(0, idx), ...layers.slice(idx + 1), layer];
  },

  /** Swap `id` with the layer directly above it in the stack. */
  bringForward(id: string): void {
    const idx = layers.findIndex((l) => l.id === id);
    if (idx < 0 || idx === layers.length - 1) return;
    const current = layers[idx];
    const above = layers[idx + 1];
    if (!current || !above) return;
    layers = [...layers.slice(0, idx), above, current, ...layers.slice(idx + 2)];
  },

  /** Swap `id` with the layer directly below it in the stack. */
  sendBackward(id: string): void {
    const idx = layers.findIndex((l) => l.id === id);
    if (idx <= 0) return;
    const current = layers[idx];
    const below = layers[idx - 1];
    if (!current || !below) return;
    layers = [...layers.slice(0, idx - 1), current, below, ...layers.slice(idx + 1)];
  },

  /** Drop `id` to the bottom of the stack. */
  sendToBack(id: string): void {
    const idx = layers.findIndex((l) => l.id === id);
    if (idx <= 0) return;
    const layer = layers[idx];
    if (!layer) return;
    layers = [layer, ...layers.slice(0, idx), ...layers.slice(idx + 1)];
  },

  /** Replace the stack from a profile's authored layers (transforms come from
   *  the persisted rects; urls load async). */
  setFromLayers(input: StandardLayer[]): void {
    layers = input.map((cl) => {
      const cached = peekSourceImage(cl.path, CANVAS_MAX_EDGE);
      return {
        id: genId(),
        path: cl.path,
        url: cached?.url ?? null,
        naturalDims: cached ? { w: cached.naturalW, h: cached.naturalH } : null,
        transform: rectToTransform(cl.image_rect_mm),
      };
    });
    for (const l of layers) {
      if (l.url) continue;
      void loadSourceImage(l.path, CANVAS_MAX_EDGE)
        .then((img) => {
          layers = layers.map((x) =>
            x.id === l.id
              ? { ...x, url: img.url, naturalDims: { w: img.naturalW, h: img.naturalH } }
              : x,
          );
        })
        .catch(() => {
          // A missing layer image surfaces via profile validity; skip silently.
        });
    }
  },

  clear(): void {
    layers = [];
  },

  /** The persisted form of the current stack — `monitor_state` is added by the
   *  caller. */
  toLayers(): StandardLayer[] {
    return layers.map((l) => ({
      path: l.path,
      image_rect_mm: {
        x_mm: l.transform.offsetMmX,
        y_mm: l.transform.offsetMmY,
        w_mm: l.transform.widthMm,
        h_mm: l.transform.heightMm,
      },
    }));
  },
};
