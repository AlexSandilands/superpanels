// Pointer + wheel + keyboard interaction state for `MonitorCanvas.svelte`.
// Owns the drag-state machine and the hover/zoom/popout runes so the
// component file stays inside the 350-line hard cap.

import { hitTest } from './draw';
import type { CanvasLayout, MonitorRect } from './types';

const DRAG_THRESHOLD_PX = 2;
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 2;

export type InteractionDeps = {
  // Reactive accessors so the class re-reads the latest values from the
  // owning component without holding stale snapshots.
  getLayout: () => CanvasLayout;
  getOffset: () => [number, number];
  getOffsetScale: () => number;
  onOffsetCommit?: (offset: [number, number]) => void;
  onResetOffset?: () => void;
  onMonitorClick?: (rect: MonitorRect) => void;
};

export class CanvasInteraction {
  zoom = $state(1);
  hoverIndex = $state<number | null>(null);
  dragging = $state(false);
  dragLiveDelta = $state<[number, number]>([0, 0]);
  popoutIndex = $state<number | null>(null);

  private dragStartPx: [number, number] | null = null;
  private deps: InteractionDeps;

  constructor(deps: InteractionDeps) {
    this.deps = deps;
  }

  popoutRect(): MonitorRect | null {
    const idx = this.popoutIndex;
    if (idx === null) return null;
    return this.deps.getLayout().monitors.find((r) => r.monitorIndex === idx) ?? null;
  }

  onPointerMove(ev: PointerEvent, canvas: HTMLCanvasElement): void {
    const [x, y] = pointerToCanvas(ev, canvas);
    if (this.dragging && this.dragStartPx) {
      this.dragLiveDelta = [ev.clientX - this.dragStartPx[0], ev.clientY - this.dragStartPx[1]];
      return;
    }
    this.hoverIndex = hitTest(this.deps.getLayout(), x, y);
  }

  onPointerDown(ev: PointerEvent, canvas: HTMLCanvasElement): void {
    if (ev.button !== 0) return;
    const layout = this.deps.getLayout();
    const [x, y] = pointerToCanvas(ev, canvas);
    if (hitTest(layout, x, y) === null && !pointInsideLayout(layout, x, y)) return;
    this.dragging = true;
    this.dragStartPx = [ev.clientX, ev.clientY];
    this.dragLiveDelta = [0, 0];
    canvas.setPointerCapture(ev.pointerId);
  }

  onPointerUp(ev: PointerEvent, canvas: HTMLCanvasElement): void {
    if (canvas.hasPointerCapture(ev.pointerId)) {
      canvas.releasePointerCapture(ev.pointerId);
    }
    if (!this.dragging || !this.dragStartPx) return;
    const moved =
      Math.abs(this.dragLiveDelta[0]) > DRAG_THRESHOLD_PX ||
      Math.abs(this.dragLiveDelta[1]) > DRAG_THRESHOLD_PX;
    const wasDragging = this.dragging;
    this.dragging = false;
    this.dragStartPx = null;
    if (wasDragging && moved) {
      const offset = this.deps.getOffset();
      const scale = this.deps.getOffsetScale();
      this.deps.onOffsetCommit?.([
        offset[0] + this.dragLiveDelta[0] / scale,
        offset[1] + this.dragLiveDelta[1] / scale,
      ]);
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
      this.deps.onResetOffset?.();
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

function pointInsideLayout(layout: CanvasLayout, x: number, y: number): boolean {
  return (
    x >= layout.offsetX &&
    x <= layout.offsetX + layout.totalW &&
    y >= layout.offsetY &&
    y <= layout.offsetY + layout.totalH
  );
}
