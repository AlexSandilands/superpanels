import { bench, describe } from 'vitest';
import { hNeighbourPairs, vNeighbourPairs } from './gaps';
import type { PreviewMonitor } from './projection';

// canvas-frame budget is 8 ms total. hNeighbourPairs / vNeighbourPairs
// run on every pointer-move redraw and are O(N³) over monitor count. This
// bench locks in the cost at N=9 — Superpanels' practical multi-monitor cap.

function pm(id: string, x: number, y: number, w = 500, h = 300): PreviewMonitor {
  return {
    id,
    name: id,
    model: '',
    refreshHz: 60,
    rotation: 0,
    nativeWmm: w,
    nativeHmm: h,
    nativePxW: 1920,
    nativePxH: 1080,
    wMm: w,
    hMm: h,
    pxW: 1920,
    pxH: 1080,
    xMm: x,
    yMm: y,
    missing: false,
  };
}

// 3×3 grid: nine monitors, eight horizontal neighbour-pairs, eight vertical.
const grid9 = (() => {
  const out: PreviewMonitor[] = [];
  for (let row = 0; row < 3; row++) {
    for (let col = 0; col < 3; col++) {
      out.push(pm(`m${row}${col}`, col * 510, row * 310));
    }
  }
  return out;
})();

describe('neighbour-pair detection at N=9', () => {
  bench('hNeighbourPairs — 3×3 grid', () => {
    hNeighbourPairs(grid9);
  });

  bench('vNeighbourPairs — 3×3 grid', () => {
    vNeighbourPairs(grid9);
  });
});
