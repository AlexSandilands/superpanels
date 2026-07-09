import { describe, expect, it } from 'vitest';
import type { Profile } from '$lib/api';
import {
  canvasFingerprint,
  canvasOverridesDirty,
  coverRectDirty,
  liveLayersDirty,
  placementsDirty,
  rectDirty,
} from './dirty';
import type { PreviewMonitor } from './preview-layout';
import type { CanvasLayer } from '$lib/stores/canvas-layers.svelte';
import {
  defaultSlideshowConfig,
  type SlideshowConfig,
  type StandardLayer,
} from '$lib/types/profile-helpers';

type RectMm = { x: number; y: number; w: number; h: number };

function liveLayer(path: string, r: RectMm): CanvasLayer {
  return {
    id: path,
    path,
    url: null,
    naturalDims: null,
    transform: { offsetMmX: r.x, offsetMmY: r.y, widthMm: r.w, heightMm: r.h },
  };
}

function persistedLayer(path: string, r: RectMm): StandardLayer {
  return { path, image_rect_mm: { x_mm: r.x, y_mm: r.y, w_mm: r.w, h_mm: r.h } };
}

function profile(monitorState: Profile['monitor_state']): Profile {
  return {
    name: 'p',
    body: {
      type: 'standard',
      layers: [{ path: '/img.png', image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 } }],
    },
    monitor_state: monitorState,
    topology: 'topo-1',
    description: null,
    created_at: '2026-05-10T00:00:00Z',
    updated_at: '2026-05-10T00:00:00Z',
    last_applied_at: null,
    backend_override: null,
  };
}

describe('canvasOverridesDirty', () => {
  it('reports clean when overrides match persisted placements', () => {
    const p = profile({
      a: { x_mm: 100, y_mm: 200 },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 100, yMm: 200 } }, p);
    expect(dirty).toBe(false);
  });

  it('reports dirty when an override position drifts past the slop tolerance', () => {
    const p = profile({
      a: { x_mm: 100, y_mm: 200 },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 105, yMm: 200 } }, p);
    expect(dirty).toBe(true);
  });

  it('reports clean for sub-millimetre drift (within slop)', () => {
    const p = profile({
      a: { x_mm: 100, y_mm: 200 },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 100.2, yMm: 200.4 } }, p);
    expect(dirty).toBe(false);
  });

  it('ignores monitors without a live override', () => {
    const p = profile({
      a: { x_mm: 0, y_mm: 0 },
      b: { x_mm: 999, y_mm: 999 },
    });
    // `b` is in the profile but not in the live overrides — that means
    // the user has hidden / unplugged it, not that they edited it.
    const dirty = canvasOverridesDirty({ a: { xMm: 0, yMm: 0 } }, p);
    expect(dirty).toBe(false);
  });

  it('reports clean for an empty profile (nothing to diff against)', () => {
    const p = profile({});
    const dirty = canvasOverridesDirty({ a: { xMm: 100, yMm: 200 } }, p);
    expect(dirty).toBe(false);
  });
});

describe('placementsDirty (explicit baseline — per-image override)', () => {
  // The slideshow's live image can carry a per-image override; the canvas
  // diffs against that baseline instead of the profile's monitor_state.
  it('reports clean when the canvas matches the override placements', () => {
    const dirty = placementsDirty({ a: { xMm: 50, yMm: 60 } }, { a: { x_mm: 50, y_mm: 60 } });
    expect(dirty).toBe(false);
  });

  it('reports dirty when the canvas drifts from the override placements', () => {
    const dirty = placementsDirty({ a: { xMm: 50, yMm: 60 } }, { a: { x_mm: 0, y_mm: 0 } });
    expect(dirty).toBe(true);
  });
});

describe('rectDirty', () => {
  const rect = { x_mm: 10, y_mm: 20, w_mm: 1000, h_mm: 600 };

  it('reports clean within the slop tolerance', () => {
    const t = { offsetMmX: 10.3, offsetMmY: 19.8, widthMm: 1000.4, heightMm: 600 };
    expect(rectDirty(t, rect)).toBe(false);
  });

  it('reports dirty when any edge drifts past the tolerance', () => {
    const t = { offsetMmX: 10, offsetMmY: 20, widthMm: 1010, heightMm: 600 };
    expect(rectDirty(t, rect)).toBe(true);
  });
});

describe('liveLayersDirty', () => {
  const r: RectMm = { x: 0, y: 0, w: 1000, h: 600 };

  it('reports clean for identical stacks', () => {
    const live = [liveLayer('/a.png', r), liveLayer('/b.png', r)];
    const persisted = [persistedLayer('/a.png', r), persistedLayer('/b.png', r)];
    expect(liveLayersDirty(live, persisted)).toBe(false);
  });

  it('reports dirty when the layer count differs', () => {
    expect(liveLayersDirty([liveLayer('/a.png', r)], [])).toBe(true);
  });

  it('reports dirty when the stacking order (and thus overlap winner) changes', () => {
    const live = [liveLayer('/b.png', r), liveLayer('/a.png', r)];
    const persisted = [persistedLayer('/a.png', r), persistedLayer('/b.png', r)];
    expect(liveLayersDirty(live, persisted)).toBe(true);
  });

  it('reports dirty when a layer rect drifts past the slop tolerance', () => {
    const live = [liveLayer('/a.png', { ...r, x: 5 })];
    const persisted = [persistedLayer('/a.png', r)];
    expect(liveLayersDirty(live, persisted)).toBe(true);
  });

  it('reports clean for sub-millimetre rect drift (within slop)', () => {
    const live = [liveLayer('/a.png', { ...r, x: 0.2, y: 0.3 })];
    const persisted = [persistedLayer('/a.png', r)];
    expect(liveLayersDirty(live, persisted)).toBe(false);
  });
});

describe('coverRectDirty (untuned slideshow image baseline)', () => {
  // One 600×340 mm monitor at the origin; cover-fit of a 16:9 image is a
  // 604.4×340 mm rect centred on it (width overhangs, height matches).
  const monitor = {
    id: 'a',
    xMm: 0,
    yMm: 0,
    wMm: 600,
    hMm: 340,
  } as PreviewMonitor;
  const dims = { w: 3840, h: 2160 };

  it('reports clean when the transform sits at the cover-fit seed', () => {
    const h = 340;
    const w = h * (dims.w / dims.h);
    const t = { offsetMmX: (600 - w) / 2, offsetMmY: 0, widthMm: w, heightMm: h };
    expect(coverRectDirty(t, [monitor], dims)).toBe(false);
  });

  it('reports dirty when the image was dragged off the cover-fit seed', () => {
    const t = { offsetMmX: 50, offsetMmY: 0, widthMm: 600, heightMm: 340 };
    expect(coverRectDirty(t, [monitor], dims)).toBe(true);
  });

  it('reports clean while natural dims are unknown', () => {
    const t = { offsetMmX: 0, offsetMmY: 0, widthMm: 600, heightMm: 340 };
    expect(coverRectDirty(t, [monitor], null)).toBe(false);
  });
});

describe('canvasFingerprint', () => {
  const transform = { offsetMmX: 0, offsetMmY: 0, widthMm: 1000, heightMm: 600 };
  const overrides = { a: { xMm: 100, yMm: 200 } };
  const layers = [liveLayer('/img.png', { x: 0, y: 0, w: 1000, h: 600 })];
  const base = () => canvasFingerprint(profile({}), overrides, layers, transform);

  it('matches itself for an unchanged canvas', () => {
    expect(canvasFingerprint(profile({}), overrides, layers, transform)).toBe(base());
  });

  it('is stable across monitor-override key order', () => {
    const forward = { a: { xMm: 1, yMm: 2 }, b: { xMm: 3, yMm: 4 } };
    const reversed = { b: { xMm: 3, yMm: 4 }, a: { xMm: 1, yMm: 2 } };
    const fp = canvasFingerprint(profile({}), forward, layers, transform);
    expect(canvasFingerprint(profile({}), reversed, layers, transform)).toBe(fp);
  });

  it('changes when a monitor moves past the slop tolerance', () => {
    const moved = { a: { xMm: 101, yMm: 200 } };
    expect(canvasFingerprint(profile({}), moved, layers, transform)).not.toBe(base());
  });

  it('ignores a sub-tolerance monitor nudge', () => {
    const nudged = { a: { xMm: 100.1, yMm: 200 } };
    expect(canvasFingerprint(profile({}), nudged, layers, transform)).toBe(base());
  });

  it('changes when a layer is resized', () => {
    const resized = [liveLayer('/img.png', { x: 0, y: 0, w: 1200, h: 600 })];
    expect(canvasFingerprint(profile({}), overrides, resized, transform)).not.toBe(base());
  });

  it('changes when a layer is swapped for a different image', () => {
    const swapped = [liveLayer('/other.png', { x: 0, y: 0, w: 1000, h: 600 })];
    expect(canvasFingerprint(profile({}), overrides, swapped, transform)).not.toBe(base());
  });

  // `refresh()` rewrites these after every Apply; keying on them would leave
  // Apply permanently lit.
  it('ignores persisted fields the canvas does not push', () => {
    const touched = profile({});
    touched.last_applied_at = '2026-05-11T00:00:00Z';
    touched.topology = 'topo-2';
    touched.monitor_state = { a: { x_mm: 999, y_mm: 999 } };
    expect(canvasFingerprint(touched, overrides, layers, transform)).toBe(base());
  });

  it('changes when a slideshow setting the daemon consumes is edited', () => {
    const slideshow = (config: SlideshowConfig): Profile => ({
      ...profile({}),
      body: {
        type: 'slideshow',
        source: {
          images: { sources: [{ type: 'folder', path: '/pics', recursive: false }] },
          config,
        },
        image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 },
      },
    });
    const fp = canvasFingerprint(slideshow(defaultSlideshowConfig()), overrides, layers, transform);
    const faster = slideshow({ ...defaultSlideshowConfig(), interval_secs: 30 });
    expect(canvasFingerprint(faster, overrides, layers, transform)).not.toBe(fp);
  });

  it('tracks the live image rect for a slideshow body', () => {
    const p: Profile = {
      ...profile({}),
      body: {
        type: 'slideshow',
        source: { images: { sources: [] }, config: defaultSlideshowConfig() },
        image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 },
      },
    };
    const fp = canvasFingerprint(p, overrides, layers, transform);
    const panned = { ...transform, offsetMmX: 50 };
    expect(canvasFingerprint(p, overrides, layers, panned)).not.toBe(fp);
  });

  it('reads an absent draft as an empty fingerprint', () => {
    expect(canvasFingerprint(null, overrides, layers, transform)).toBe('');
  });
});
