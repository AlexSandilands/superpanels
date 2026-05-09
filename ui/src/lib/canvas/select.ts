// Selection-scoped canvas mutations: rotate / nudge the currently-selected
// monitor by deltas. No-op when nothing is selected.

import { canvasView } from '$lib/stores/canvas-view.svelte';

export function rotateSelected(deltaDeg: number): void {
  const id = canvasView.selectId;
  if (!id) return;
  const cur = canvasView.overrides[id];
  if (!cur) return;
  const next = (((cur.rotation + deltaDeg) % 360) + 360) % 360;
  canvasView.override(id, { rotation: next as 0 | 90 | 180 | 270 });
}

export function nudgeSelected(dxMm: number, dyMm: number): void {
  const id = canvasView.selectId;
  if (!id) return;
  const cur = canvasView.overrides[id];
  if (!cur) return;
  canvasView.override(id, { xMm: cur.xMm + dxMm, yMm: cur.yMm + dyMm });
}
