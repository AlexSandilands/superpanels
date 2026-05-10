import { describe, expect, it } from 'vitest';
import type { Profile } from '$lib/api';
import { canvasOverridesDirty } from './dirty';

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
      a: { x_mm: 100, y_mm: 200, rotation: 'none' },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 100, yMm: 200, rotation: 0 } }, p);
    expect(dirty).toBe(false);
  });

  it('reports dirty when an override position drifts past the slop tolerance', () => {
    const p = profile({
      a: { x_mm: 100, y_mm: 200, rotation: 'none' },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 105, yMm: 200, rotation: 0 } }, p);
    expect(dirty).toBe(true);
  });

  it('reports clean for sub-millimetre drift (within slop)', () => {
    const p = profile({
      a: { x_mm: 100, y_mm: 200, rotation: 'none' },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 100.2, yMm: 200.4, rotation: 0 } }, p);
    expect(dirty).toBe(false);
  });

  it('reports dirty when a rotation differs', () => {
    const p = profile({
      a: { x_mm: 0, y_mm: 0, rotation: 'none' },
    });
    const dirty = canvasOverridesDirty({ a: { xMm: 0, yMm: 0, rotation: 90 } }, p);
    expect(dirty).toBe(true);
  });

  it('ignores monitors without a live override', () => {
    const p = profile({
      a: { x_mm: 0, y_mm: 0, rotation: 'none' },
      b: { x_mm: 999, y_mm: 999, rotation: 'right' },
    });
    // `b` is in the profile but not in the live overrides — that means
    // the user has hidden / unplugged it, not that they edited it.
    const dirty = canvasOverridesDirty({ a: { xMm: 0, yMm: 0, rotation: 0 } }, p);
    expect(dirty).toBe(false);
  });

  it('reports clean for an empty profile (nothing to diff against)', () => {
    const p = profile({});
    const dirty = canvasOverridesDirty({ a: { xMm: 100, yMm: 200, rotation: 0 } }, p);
    expect(dirty).toBe(false);
  });

  it('rotation mapping covers all four cardinal values', () => {
    // Lock in the 'none' / 'right' / 'inverted' / 'left' → 0/90/180/270
    // table; a regression here would silently flag profiles dirty.
    const baseProfile = profile({
      n: { x_mm: 0, y_mm: 0, rotation: 'none' },
      r: { x_mm: 0, y_mm: 0, rotation: 'right' },
      i: { x_mm: 0, y_mm: 0, rotation: 'inverted' },
      l: { x_mm: 0, y_mm: 0, rotation: 'left' },
    });
    const matching = canvasOverridesDirty(
      {
        n: { xMm: 0, yMm: 0, rotation: 0 },
        r: { xMm: 0, yMm: 0, rotation: 90 },
        i: { xMm: 0, yMm: 0, rotation: 180 },
        l: { xMm: 0, yMm: 0, rotation: 270 },
      },
      baseProfile,
    );
    expect(matching).toBe(false);
  });
});
