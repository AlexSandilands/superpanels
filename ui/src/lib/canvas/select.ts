// Selection-scoped canvas mutations: nudge the currently-selected monitor
// by deltas. No-op when nothing is selected. Rotation isn't user-authored —
// it tracks the compositor via `Monitor.rotation`.

import { canvasView } from '$lib/stores/canvas-view.svelte';

export function nudgeSelected(dxMm: number, dyMm: number): void {
  const id = canvasView.selectId;
  if (!id) return;
  const cur = canvasView.overrides[id];
  if (!cur) return;
  canvasView.override(id, { xMm: cur.xMm + dxMm, yMm: cur.yMm + dyMm });
}
