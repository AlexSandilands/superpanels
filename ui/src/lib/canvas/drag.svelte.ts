// Drag state machine for the preview canvas. The component owns layout +
// rendering; this module owns the per-pointer-down→move→up state transitions
// and emits committed deltas via the supplied callbacks.

import { canvasView } from '$lib/stores/canvas-view.svelte';
import { monitorRect, type PreviewMonitor, type Rect } from '$lib/canvas/preview-layout';
import type { ImageTransform } from '$lib/stores/image-transform.svelte';

export type Drag =
  | { kind: 'image'; startX: number; startY: number; startMmX: number; startMmY: number }
  | {
      kind: 'image-resize';
      corner: 'br' | 'tl';
      startX: number;
      startMmX: number;
      startMmY: number;
      startW: number;
      startH: number;
      aspect: number;
    }
  | {
      kind: 'layer-image';
      id: string;
      startX: number;
      startY: number;
      startMmX: number;
      startMmY: number;
    }
  | {
      kind: 'layer-resize';
      id: string;
      corner: 'br' | 'tl';
      startX: number;
      startMmX: number;
      startMmY: number;
      startW: number;
      startH: number;
      aspect: number;
    }
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

export type ResizeDrag = {
  corner: 'br' | 'tl';
  startMmX: number;
  startMmY: number;
  startW: number;
  startH: number;
  aspect: number;
};

// Aspect-locked resize driven by horizontal drag. The bottom-right handle keeps
// the top-left anchored (offset unchanged); the top-left handle keeps the
// bottom-right anchored, so the opposite corner stays put as the rect grows.
export function resizeRect(
  d: ResizeDrag,
  dxMm: number,
): { offsetMmX: number; offsetMmY: number; widthMm: number; heightMm: number } {
  const widthMm = Math.max(50, d.corner === 'tl' ? d.startW - dxMm : d.startW + dxMm);
  const heightMm = widthMm / d.aspect;
  if (d.corner === 'tl') {
    return {
      offsetMmX: d.startMmX + d.startW - widthMm,
      offsetMmY: d.startMmY + d.startH - heightMm,
      widthMm,
      heightMm,
    };
  }
  return { offsetMmX: d.startMmX, offsetMmY: d.startMmY, widthMm, heightMm };
}

// Snap a free-floating layer rect so its edges align to any monitor edge, within
// `dist` mm. Pure core of the drag controller's `snapRect` — extracted so the
// edge-matching math is unit-testable without a live pointer gesture.
export function snapRectToMonitors(
  x: number,
  y: number,
  w: number,
  h: number,
  monitors: PreviewMonitor[],
  dist: number,
): { x: number; y: number; guides: Guide[] } {
  const out: Guide[] = [];
  let nx = x;
  let ny = y;
  for (const m of monitors) {
    const r = monitorRect(m);
    for (const edge of [r.x, r.x + r.w]) {
      if (Math.abs(x - edge) < dist) {
        nx = edge;
        out.push({ kind: 'v', x: edge });
      }
      if (Math.abs(x + w - edge) < dist) {
        nx = edge - w;
        out.push({ kind: 'v', x: edge });
      }
    }
    for (const edge of [r.y, r.y + r.h]) {
      if (Math.abs(y - edge) < dist) {
        ny = edge;
        out.push({ kind: 'h', y: edge });
      }
      if (Math.abs(y + h - edge) < dist) {
        ny = edge - h;
        out.push({ kind: 'h', y: edge });
      }
    }
  }
  return { x: nx, y: ny, guides: out };
}

export type DragHandlers = {
  monitors: () => PreviewMonitor[];
  imageTransform: () => ImageTransform;
  scale: () => number;
  bezelHmm: () => number;
  setImageTransform: (t: ImageTransform) => void;
  // Composite mode — resolve / write a layer's transform by id. No-ops in the
  // single-image span path, which never begins a layer drag.
  getLayer?: (id: string) => ImageTransform | null;
  setLayerTransform?: (id: string, t: ImageTransform) => void;
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

  // Snap a free-floating layer rect so its edges align to any monitor edge —
  // the gesture the user means by "snap the image into place on that monitor".
  function snapRect(
    x: number,
    y: number,
    w: number,
    h: number,
  ): { x: number; y: number; guides: Guide[] } {
    return snapRectToMonitors(x, y, w, h, handlers.monitors(), 8 / handlers.scale());
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
      const dy =
        drag.kind === 'image-resize' || drag.kind === 'layer-resize' ? 0 : ev.clientY - drag.startY;
      const dyMm = dy / scale;

      if (drag.kind === 'image') {
        handlers.setImageTransform({
          ...handlers.imageTransform(),
          offsetMmX: drag.startMmX + dxMm,
          offsetMmY: drag.startMmY + dyMm,
        });
      } else if (drag.kind === 'image-resize') {
        const r = resizeRect(drag, dxMm);
        handlers.setImageTransform({ ...handlers.imageTransform(), ...r });
      } else if (drag.kind === 'layer-image') {
        const t = handlers.getLayer?.(drag.id);
        if (!t) return;
        let nx = drag.startMmX + dxMm;
        let ny = drag.startMmY + dyMm;
        if (!ev.altKey) {
          const snapped = snapRect(nx, ny, t.widthMm, t.heightMm);
          nx = snapped.x;
          ny = snapped.y;
          guides = snapped.guides;
        } else {
          guides = [];
        }
        handlers.setLayerTransform?.(drag.id, { ...t, offsetMmX: nx, offsetMmY: ny });
      } else if (drag.kind === 'layer-resize') {
        const t = handlers.getLayer?.(drag.id);
        if (!t) return;
        handlers.setLayerTransform?.(drag.id, { ...t, ...resizeRect(drag, dxMm) });
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
