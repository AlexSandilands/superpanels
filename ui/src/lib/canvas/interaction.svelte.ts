// Pointer + wheel + keyboard interaction state for `MonitorCanvas.svelte`.
// Phase 4c free-positioning: drag-to-pan + drag-corner-to-resize, plus the
// `D` key to toggle off-monitor dimming and `R` to reset the transform.

import { hitResizeHandle, hitTest, pointInsideRect, type ResizeCorner } from './draw';
import type { CanvasLayout, MonitorRect } from './types';

const DRAG_THRESHOLD_PX = 2;
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 2;
const MIN_IMAGE_SIZE_PX = 32;

type ImgRect = { x: number; y: number; w: number; h: number } | null;

export type InteractionDeps = {
  // Reactive accessors so the class re-reads the latest values from the
  // owning component without holding stale snapshots.
  getLayout: () => CanvasLayout;
  getOffset: () => [number, number];
  /** Display-px image rectangle, derived from offset + image_size_px or FitMode. */
  getImageRect: () => ImgRect;
  /** Display-px → core-canvas-px factor for persisted values. */
  getOffsetScale: () => number;
  /** `null` when image_size_px is unset (FitMode-driven). */
  getImageSizePx: () => [number, number] | null;
  /** Aspect ratio to lock during corner resize (typically image natural aspect). */
  getAspectLock: () => boolean;
  onTransformCommit?: (offset: [number, number], imageSizePx: [number, number] | null) => void;
  onResetTransform?: () => void;
  onMonitorClick?: (rect: MonitorRect) => void;
};

type DragMode =
  | { kind: 'pan'; startClient: [number, number] }
  | { kind: 'resize'; corner: ResizeCorner; startClient: [number, number] };

export class CanvasInteraction {
  zoom = $state(1);
  hoverIndex = $state<number | null>(null);
  dragging = $state(false);
  dragLiveDelta = $state<[number, number]>([0, 0]);
  dragMode = $state<DragMode | null>(null);
  popoutIndex = $state<number | null>(null);
  dim = $state(true);

  private deps: InteractionDeps;

  constructor(deps: InteractionDeps) {
    this.deps = deps;
  }

  popoutRect(): MonitorRect | null {
    const idx = this.popoutIndex;
    if (idx === null) return null;
    return this.deps.getLayout().monitors.find((r) => r.monitorIndex === idx) ?? null;
  }

  cursorForResize(corner: ResizeCorner | null): string | null {
    if (corner === 'tl' || corner === 'br') return 'nwse-resize';
    if (corner === 'tr' || corner === 'bl') return 'nesw-resize';
    return null;
  }

  hitResizeHandle(x: number, y: number): ResizeCorner | null {
    const rect = this.deps.getImageRect();
    if (!rect) return null;
    return hitResizeHandle(rect, x, y);
  }

  onPointerMove(ev: PointerEvent, canvas: HTMLCanvasElement): void {
    const [x, y] = pointerToCanvas(ev, canvas);
    if (this.dragging && this.dragMode) {
      this.dragLiveDelta = [
        ev.clientX - this.dragMode.startClient[0],
        ev.clientY - this.dragMode.startClient[1],
      ];
      return;
    }
    this.hoverIndex = hitTest(this.deps.getLayout(), x, y);
  }

  onPointerDown(ev: PointerEvent, canvas: HTMLCanvasElement): void {
    if (ev.button !== 0) return;
    const layout = this.deps.getLayout();
    const [x, y] = pointerToCanvas(ev, canvas);
    const imgRect = this.deps.getImageRect();
    // Hit priority: corner handles → image body → monitor rect.
    const corner = imgRect ? hitResizeHandle(imgRect, x, y) : null;
    if (corner) {
      this.beginDrag(canvas, ev.pointerId, ev.clientX, ev.clientY, {
        kind: 'resize',
        corner,
        startClient: [ev.clientX, ev.clientY],
      });
      return;
    }
    if (imgRect && pointInsideRect(imgRect, x, y)) {
      this.beginDrag(canvas, ev.pointerId, ev.clientX, ev.clientY, {
        kind: 'pan',
        startClient: [ev.clientX, ev.clientY],
      });
      return;
    }
    // Falls through to the popout-on-up flow when releasing on a monitor.
    if (hitTest(layout, x, y) !== null) {
      this.beginDrag(canvas, ev.pointerId, ev.clientX, ev.clientY, {
        kind: 'pan',
        startClient: [ev.clientX, ev.clientY],
      });
    }
  }

  private beginDrag(
    canvas: HTMLCanvasElement,
    pointerId: number,
    clientX: number,
    clientY: number,
    mode: DragMode,
  ): void {
    this.dragging = true;
    this.dragMode = mode;
    this.dragLiveDelta = [0, 0];
    canvas.setPointerCapture(pointerId);
    void clientX;
    void clientY;
  }

  onPointerUp(ev: PointerEvent, canvas: HTMLCanvasElement): void {
    if (canvas.hasPointerCapture(ev.pointerId)) {
      canvas.releasePointerCapture(ev.pointerId);
    }
    if (!this.dragging || !this.dragMode) return;
    const moved =
      Math.abs(this.dragLiveDelta[0]) > DRAG_THRESHOLD_PX ||
      Math.abs(this.dragLiveDelta[1]) > DRAG_THRESHOLD_PX;
    const mode = this.dragMode;
    this.dragging = false;
    this.dragMode = null;
    if (moved) {
      this.commitDrag(mode);
    } else {
      const layout = this.deps.getLayout();
      const [x, y] = pointerToCanvas(ev, canvas);
      const idx = hitTest(layout, x, y);
      if (idx !== null) {
        this.popoutIndex = this.popoutIndex === idx ? null : idx;
        const rect = layout.monitors.find((r) => r.monitorIndex === idx);
        if (rect) this.deps.onMonitorClick?.(rect);
      } else {
        this.popoutIndex = null;
      }
    }
    this.dragLiveDelta = [0, 0];
  }

  private commitDrag(mode: DragMode): void {
    const offset = this.deps.getOffset();
    const scale = this.deps.getOffsetScale();
    const dx = this.dragLiveDelta[0] / scale;
    const dy = this.dragLiveDelta[1] / scale;
    if (mode.kind === 'pan') {
      this.deps.onTransformCommit?.([offset[0] + dx, offset[1] + dy], this.deps.getImageSizePx());
      return;
    }
    const next = this.computeResize(mode.corner, dx, dy);
    if (!next) return;
    this.deps.onTransformCommit?.(next.offset, next.imageSizePx);
  }

  private computeResize(
    corner: ResizeCorner,
    dx: number,
    dy: number,
  ): { offset: [number, number]; imageSizePx: [number, number] } | null {
    const baseSizePx = this.resolveBaseSizePx();
    if (!baseSizePx) return null;
    const baseOffset = this.deps.getOffset();
    const aspect = this.deps.getAspectLock() ? baseSizePx[0] / baseSizePx[1] : null;
    let { x, y, w, h } = {
      x: baseOffset[0],
      y: baseOffset[1],
      w: baseSizePx[0],
      h: baseSizePx[1],
    };
    // Apply the corner delta. Origins move with the dragged corner so the
    // opposite corner stays anchored.
    if (corner === 'br') {
      w = Math.max(MIN_IMAGE_SIZE_PX, w + dx);
      h = Math.max(MIN_IMAGE_SIZE_PX, h + dy);
    } else if (corner === 'tr') {
      w = Math.max(MIN_IMAGE_SIZE_PX, w + dx);
      const newH = Math.max(MIN_IMAGE_SIZE_PX, h - dy);
      y += h - newH;
      h = newH;
    } else if (corner === 'bl') {
      const newW = Math.max(MIN_IMAGE_SIZE_PX, w - dx);
      x += w - newW;
      w = newW;
      h = Math.max(MIN_IMAGE_SIZE_PX, h + dy);
    } else if (corner === 'tl') {
      const newW = Math.max(MIN_IMAGE_SIZE_PX, w - dx);
      const newH = Math.max(MIN_IMAGE_SIZE_PX, h - dy);
      x += w - newW;
      y += h - newH;
      w = newW;
      h = newH;
    }
    if (aspect && Number.isFinite(aspect) && aspect > 0) {
      // Keep the aspect by treating width as authoritative and rebuilding
      // height around the anchored corner.
      const newH = w / aspect;
      if (corner === 'tl' || corner === 'tr') {
        y += h - newH;
      }
      h = newH;
    }
    return {
      offset: [Math.round(x), Math.round(y)],
      imageSizePx: [Math.max(1, Math.round(w)), Math.max(1, Math.round(h))],
    };
  }

  private resolveBaseSizePx(): [number, number] | null {
    const explicit = this.deps.getImageSizePx();
    if (explicit) return explicit;
    // Derive from the current display rect divided by the offsetScale so the
    // first resize after a FitMode-driven render lands at the visible size.
    const rect = this.deps.getImageRect();
    if (!rect) return null;
    const scale = this.deps.getOffsetScale();
    if (!Number.isFinite(scale) || scale <= 0) return null;
    return [rect.w / scale, rect.h / scale];
  }

  onPointerLeave(): void {
    if (this.dragging) return;
    this.hoverIndex = null;
  }

  onWheel(ev: WheelEvent): void {
    ev.preventDefault();
    const factor = Math.exp(-ev.deltaY * 0.0015);
    this.zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, this.zoom * factor));
  }

  onKey(ev: KeyboardEvent): void {
    const target = ev.target as HTMLElement | null;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
    if (ev.key === 'r' || ev.key === 'R') {
      ev.preventDefault();
      this.zoom = 1;
      this.deps.onResetTransform?.();
    } else if (ev.key === 'd' || ev.key === 'D') {
      ev.preventDefault();
      this.dim = !this.dim;
    } else if (ev.key === 'Escape' && this.popoutIndex !== null) {
      ev.preventDefault();
      this.popoutIndex = null;
    }
  }

  closePopout(): void {
    this.popoutIndex = null;
  }
}

function pointerToCanvas(ev: PointerEvent, canvas: HTMLCanvasElement): [number, number] {
  const rect = canvas.getBoundingClientRect();
  return [ev.clientX - rect.left, ev.clientY - rect.top];
}
