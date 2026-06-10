import { describe, expect, it } from 'vitest';
import type { Profile } from '$lib/api';
import { canvasOverridesDirty, coverRectDirty, placementsDirty, rectDirty } from './dirty';
import type { PreviewMonitor } from './preview-layout';

function profile(monitorState: Profile['monitor_state']): Profile {
  return {
    name: 'p',
    body: {
      type: 'span',
      source: { type: 'single', path: '/img.png' },
      image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 },
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
