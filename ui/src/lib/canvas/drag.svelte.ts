// Drag state machine for the preview canvas. The component owns layout +
// rendering; this module owns the per-pointer-down→move→up state transitions
// and emits committed deltas via the supplied callbacks.

import { canvasView } from '$lib/stores/canvas-view.svelte';
import { monitorRect, type PreviewMonitor, type Rect } from '$lib/canvas/preview-layout';
import type { ImageTransform } from '$lib/stores/image-transform.svelte';

export type Drag =
  | { kind: 'image'; startX: number; startY: number; startMmX: number; startMmY: number }
  | { kind: 'image-resize'; startX: number; startW: number; startH: number; aspect: number }
  | {
      kind: 'monitor';
      id: string;
      startX: number;
      startY: number;
      startMmX: number;
      startMmY: number;
    }
  | { kind: 'pan'; startX: number; startY: number; startOx: number; startOy: number };

export type Guide = { kind: 'h'; y: number } | { kind: 'v'; x: number };

export type DragHandlers = {
  monitors: () => PreviewMonitor[];
  imageTransform: () => ImageTransform;
  scale: () => number;
  bezelHmm: () => number;
  setImageTransform: (t: ImageTransform) => void;
};

export function createDragController(handlers: DragHandlers) {
  let drag = $state<Drag | null>(null);
  let guides = $state<Guide[]>([]);

  function snap(id: string, x: number, y: number): { x: number; y: number; guides: Guide[] } {
    const monitors = handlers.monitors();
    const me = monitors.find((m) => m.id === id);
    if (!me) return { x, y, guides: [] };
    const meR: Rect = { x, y, w: me.wMm, h: me.hMm };
    const dist = 8 / handlers.scale();
    const out: Guide[] = [];
    let nx = x;
    let ny = y;
    const bezelHmm = handlers.bezelHmm();
    for (const o of monitors) {
      if (o.id === id) continue;
      const oR = monitorRect(o);
      if (Math.abs(meR.y - oR.y) < dist) {
        ny = oR.y;
        out.push({ kind: 'h', y: oR.y });
      }
      if (Math.abs(meR.y + meR.h - (oR.y + oR.h)) < dist) {
        ny = oR.y + oR.h - meR.h;
        out.push({ kind: 'h', y: oR.y + oR.h });
      }
      if (Math.abs(meR.x - oR.x) < dist) {
        nx = oR.x;
        out.push({ kind: 'v', x: oR.x });
      }
      if (Math.abs(meR.x + meR.w - (oR.x + oR.w)) < dist) {
        nx = oR.x + oR.w - meR.w;
        out.push({ kind: 'v', x: oR.x + oR.w });
      }
      if (Math.abs(meR.x - (oR.x + oR.w + bezelHmm)) < dist) nx = oR.x + oR.w + bezelHmm;
      if (Math.abs(meR.x + meR.w - (oR.x - bezelHmm)) < dist) nx = oR.x - bezelHmm - meR.w;
    }
    return { x: nx, y: ny, guides: out };
  }

  return {
    get drag() {
      return drag;
    },
    get guides() {
      return guides;
    },
    begin(next: Drag) {
      drag = next;
    },
    end() {
      drag = null;
      guides = [];
    },
    move(ev: PointerEvent) {
      if (!drag) return;
      const dx = ev.clientX - drag.startX;
      const scale = handlers.scale();
      const dxMm = dx / scale;
      const dy = drag.kind === 'image-resize' ? 0 : ev.clientY - drag.startY;
      const dyMm = dy / scale;

      if (drag.kind === 'image') {
        handlers.setImageTransform({
          ...handlers.imageTransform(),
          offsetMmX: drag.startMmX + dxMm,
          offsetMmY: drag.startMmY + dyMm,
        });
      } else if (drag.kind === 'image-resize') {
        const newW = Math.max(50, drag.startW + dxMm);
        const newH = newW / drag.aspect;
        handlers.setImageTransform({ ...handlers.imageTransform(), widthMm: newW, heightMm: newH });
      } else if (drag.kind === 'monitor') {
        let newX = drag.startMmX + dxMm;
        let newY = drag.startMmY + dyMm;
        if (!ev.altKey) {
          const snapped = snap(drag.id, newX, newY);
          newX = snapped.x;
          newY = snapped.y;
          guides = snapped.guides;
        } else {
          guides = [];
        }
        canvasView.override(drag.id, { xMm: newX, yMm: newY });
      } else if (drag.kind === 'pan') {
        canvasView.setPan(drag.startOx + dx, drag.startOy + dy);
      }
    },
  };
}
