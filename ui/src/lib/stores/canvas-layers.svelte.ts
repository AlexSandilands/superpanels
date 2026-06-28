// Composite-mode canvas state: an ordered list of free-positioned images
// (layers) in mm-space, index 0 = bottom. The single-image span path uses
// `image-transform.svelte`; this is its multi-image sibling. Each layer carries
// its own loaded `url` / `naturalDims` (resolved async) and `transform`.

import { errorMessage } from '$lib/api';
import { coverImageRect, type PreviewMonitor } from '$lib/canvas/preview-layout';
import { loadSourceImage, peekSourceImage } from '$lib/library/source-image';
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

  /** Append a new image on top of the stack, cover-fit over `monitors` at the
   *  image's aspect once it resolves. */
  async add(path: string, monitors: PreviewMonitor[]): Promise<void> {
    const id = genId();
    const stagger = layers.length * STAGGER_MM;
    const cached = peekSourceImage(path);
    const layer: CanvasLayer = {
      id,
      path,
      url: cached?.url ?? null,
      naturalDims: cached ? { w: cached.naturalW, h: cached.naturalH } : null,
      transform: coverTransform(
        monitors,
        cached ? cached.naturalW / cached.naturalH : 16 / 9,
        stagger,
      ),
    };
    layers = [...layers, layer];
    if (cached) return;
    try {
      const img = await loadSourceImage(path);
      // Re-seed at the true aspect now that we know it (the layer was invisible
      // until now — no url — so the user can't have dragged it yet).
      layers = layers.map((l) =>
        l.id === id
          ? {
              ...l,
              url: img.url,
              naturalDims: { w: img.naturalW, h: img.naturalH },
              transform: coverTransform(monitors, img.naturalW / img.naturalH, stagger),
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

  /** Replace the stack from a profile's authored layers (transforms come from
   *  the persisted rects; urls load async). */
  setFromLayers(input: StandardLayer[]): void {
    layers = input.map((cl) => {
      const cached = peekSourceImage(cl.path);
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
      void loadSourceImage(l.path)
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
