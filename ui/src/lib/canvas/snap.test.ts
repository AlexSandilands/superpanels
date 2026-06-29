import { describe, it, expect } from 'vitest';
import { contain, cover, fitHeight, fitWidth, targetRectForLayer } from './snap';
import type { ImageTransform } from '$lib/stores/image-transform.svelte';
import type { PreviewMonitor } from './preview-layout';

function pm(over: Partial<PreviewMonitor> & { id: string }): PreviewMonitor {
  const base: PreviewMonitor = {
    id: over.id,
    name: over.id,
    model: '',
    refreshHz: 60,
    rotation: 0,
    nativeWmm: over.wMm ?? 500,
    nativeHmm: over.hMm ?? 300,
    nativePxW: 1920,
    nativePxH: 1080,
    wMm: over.wMm ?? 500,
    hMm: over.hMm ?? 300,
    pxW: 1920,
    pxH: 1080,
    xMm: over.xMm ?? 0,
    yMm: over.yMm ?? 0,
    missing: false,
  };
  return { ...base, ...over };
}

function layer(
  offsetMmX: number,
  offsetMmY: number,
  widthMm: number,
  heightMm: number,
): ImageTransform {
  return { offsetMmX, offsetMmY, widthMm, heightMm };
}

describe('targetRectForLayer', () => {
  it('returns null when there are no monitors', () => {
    expect(targetRectForLayer(layer(0, 0, 100, 100), [])).toBeNull();
  });

  it('returns null when the layer overlaps nothing', () => {
    const m = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    expect(targetRectForLayer(layer(2000, 2000, 100, 100), [m])).toBeNull();
  });

  it('bounds every monitor covered past the 50% threshold', () => {
    // Layer fully covers both side-by-side monitors → bbox spans both.
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 500, yMm: 0, wMm: 500, hMm: 300 });
    const target = targetRectForLayer(layer(0, 0, 1000, 300), [a, b]);
    expect(target).toEqual({ x: 0, y: 0, w: 1000, h: 300 });
  });

  it('ignores a monitor it barely clips (below threshold)', () => {
    // Covers all of A but only a 50mm sliver of B (< 50% of B's 500mm width).
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 500, yMm: 0, wMm: 500, hMm: 300 });
    const target = targetRectForLayer(layer(0, 0, 550, 300), [a, b]);
    expect(target).toEqual({ x: 0, y: 0, w: 500, h: 300 });
  });

  it('falls back to the single most-overlapped monitor when none reach the threshold', () => {
    // Layer straddles A and B but covers < 50% of each; B has more overlap.
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 500, yMm: 0, wMm: 500, hMm: 300 });
    const target = targetRectForLayer(layer(400, 0, 300, 300), [a, b]);
    expect(target).toEqual({ x: 500, y: 0, w: 500, h: 300 });
  });
});

describe('fit helpers', () => {
  const target = { x: 100, y: 50, w: 400, h: 200 };

  it('fitWidth spans the target width and centers vertically', () => {
    const t = fitWidth(target, 1); // square image into 400x200 → 400x400
    expect(t.widthMm).toBe(400);
    expect(t.heightMm).toBe(400);
    expect(t.offsetMmX).toBe(100);
    expect(t.offsetMmY).toBe(50 + (200 - 400) / 2);
  });

  it('fitHeight spans the target height and centers horizontally', () => {
    const t = fitHeight(target, 1); // square image into 400x200 → 200x200
    expect(t.widthMm).toBe(200);
    expect(t.heightMm).toBe(200);
    expect(t.offsetMmY).toBe(50);
    expect(t.offsetMmX).toBe(100 + (400 - 200) / 2);
  });

  it('cover picks the axis fit that fully covers the target', () => {
    // Square image over a 400x200 (wide) target: width-fit (400x400) covers
    // vertically, so cover == fitWidth.
    expect(cover(target, 1)).toEqual(fitWidth(target, 1));
  });

  it('cover uses the height fit when width-fit would not span vertically', () => {
    // Wide image (aspect 4) over the same target: width-fit is 400x100 (does
    // not cover the 200 height), so cover falls to fitHeight.
    expect(cover(target, 4)).toEqual(fitHeight(target, 4));
  });

  it('contain uses the width fit when the image is wider than the target', () => {
    // Wide image (aspect 4) over the 400x200 target: width-fit is 400x100,
    // which fits inside, so contain == fitWidth (letterboxed top/bottom).
    expect(contain(target, 4)).toEqual(fitWidth(target, 4));
  });

  it('contain uses the height fit when the image is taller than the target', () => {
    // Square image over the 400x200 (wide) target: width-fit (400x400) spills
    // past the height, so contain falls to fitHeight (pillarboxed left/right).
    expect(contain(target, 1)).toEqual(fitHeight(target, 1));
  });
});
