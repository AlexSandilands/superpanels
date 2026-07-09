// Window-move regions published to the Rust side, which starts the drag from
// the GTK button press itself. See `crates/superpanels-gui/src/window_chrome.rs`
// for why the move can't be started from a `mousedown` handler.

import { api, type DragRect } from './api';

export type { DragRect };

export const DRAG_ATTR = 'data-drag-region';

/** Rects of every drag region under `root`, in window-relative CSS pixels. */
export function measureDragRegions(root: ParentNode): DragRect[] {
  return Array.from(root.querySelectorAll(`[${DRAG_ATTR}]`))
    .map((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.left, y: r.top, w: r.width, h: r.height };
    })
    .filter((r) => r.w > 0 && r.h > 0);
}

/** Publishes regions to the backend, skipping unchanged sets — the titlebar
 *  re-measures on every clock tick, and most ticks move nothing. */
export function createDragRegionPublisher(
  send: (regions: DragRect[]) => Promise<unknown> = (r) => api.setDragRegions(r),
): (regions: DragRect[]) => void {
  let last: string | null = null;
  return (regions) => {
    const key = JSON.stringify(regions);
    if (key === last) return;
    last = key;
    void send(regions).catch(() => {
      // Retry on the next tick rather than stranding the backend on a stale set.
      last = null;
    });
  };
}
