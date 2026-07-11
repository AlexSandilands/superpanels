import { describe, it, expect } from 'vitest';
import { panCommit, resizeRect, snapRectToMonitors, type ResizeDrag } from './drag.svelte';
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

const baseDrag: ResizeDrag = {
  corner: 'br',
  startMmX: 100,
  startMmY: 50,
  startW: 200,
  startH: 100,
  aspect: 2,
};

describe('resizeRect', () => {
  it('grows from the bottom-right with the top-left anchored, aspect locked', () => {
    const r = resizeRect(baseDrag, 40);
    expect(r).toEqual({ offsetMmX: 100, offsetMmY: 50, widthMm: 240, heightMm: 120 });
  });

  it('grows from the top-left with the bottom-right anchored', () => {
    const r = resizeRect({ ...baseDrag, corner: 'tl' }, -40);
    // width = startW - dxMm = 240; opposite (bottom-right) corner stays at (300, 150).
    expect(r.widthMm).toBe(240);
    expect(r.heightMm).toBe(120);
    expect(r.offsetMmX).toBe(60);
    expect(r.offsetMmY).toBe(30);
    expect(r.offsetMmX + r.widthMm).toBe(300);
    expect(r.offsetMmY + r.heightMm).toBe(150);
  });

  it('clamps width to a 50mm floor and keeps height on the aspect', () => {
    const r = resizeRect({ ...baseDrag, startW: 60 }, -100);
    expect(r.widthMm).toBe(50);
    expect(r.heightMm).toBe(25); // 50 / aspect 2
  });
});

describe('snapRectToMonitors', () => {
  const monitor = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });

  it('snaps a near left edge to the monitor edge and emits a guide', () => {
    const out = snapRectToMonitors(3, 0, 100, 100, [monitor], 8);
    expect(out.x).toBe(0);
    expect(out.guides).toContainEqual({ kind: 'v', x: 0 });
  });

  it('snaps a near right edge so the rect ends on the monitor edge', () => {
    const out = snapRectToMonitors(403, 0, 100, 100, [monitor], 8);
    expect(out.x).toBe(400); // 500 - width
    expect(out.guides).toContainEqual({ kind: 'v', x: 500 });
  });

  it('leaves the rect unmoved when no edge is within range', () => {
    const out = snapRectToMonitors(100, 100, 100, 100, [monitor], 8);
    expect(out.x).toBe(100);
    expect(out.y).toBe(100);
    expect(out.guides).toHaveLength(0);
  });
});

describe('panCommit', () => {
  it('folds the live pan offset into the committed origin', () => {
    expect(panCommit(30, -10, { x: 5, y: 12 })).toEqual({ x: 35, y: 2 });
  });

  it('is identity for a zero offset (a click with no drag)', () => {
    expect(panCommit(30, -10, { x: 0, y: 0 })).toEqual({ x: 30, y: -10 });
  });
});
