// Pure pointer hit-testing for the preview canvas. Given projected pixel rects,
// resolve what's under a point — composite layers (top-down, above monitors),
// then monitors, then the single span image. Kept out of the component so the
// layer-vs-monitor z-order priority is unit-testable.

export type Rect = { x: number; y: number; w: number; h: number };

export type Hit =
  | { type: 'monitor'; id: string }
  | { type: 'image' }
  | { type: 'image-resize' }
  | { type: 'layer'; id: string }
  | { type: 'layer-resize'; id: string }
  | { type: 'layer-remove'; id: string }
  | { type: 'layer-snap'; id: string; axis: 'width' | 'height' }
  | { type: 'stage' };

export type HitGeometry = {
  compositeMode: boolean;
  /** When false, layers and the span image are click-through and pointer
   *  gestures fall to the monitors beneath them. */
  imagesInteractive: boolean;
  /** Layers small enough to hide their on-image buttons skip the snap/remove
   *  hit regions (the toolbar mirrors the actions). */
  buttonsForLayer: (id: string) => boolean;
  layerRects: Array<{ id: string; rect: Rect }>;
  monitors: Array<{ id: string; rect: Rect }>;
  imageUrl: string | null;
  imgRect: Rect;
};

const REMOVE_HIT_R = 13;
const SNAP_HIT_R = 13;
const RESIZE_HIT_R = 18;
const IMAGE_RESIZE_HIT_R = 24;

// On-layer button cluster, top-right, laid out right→left: remove, then the two
// snap buttons. Centres are measured in from the layer's top-right corner.
const BTN_TOP = 20;
const REMOVE_CX = 12;
const SNAP_W_CX = 38;
const SNAP_H_CX = 64;

function inRect(px: number, py: number, r: Rect): boolean {
  return px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h;
}

export function hitTest(px: number, py: number, geo: HitGeometry): Hit {
  // Composite layers sit above the monitors; test them top-down first — unless
  // the canvas is in monitor mode, where layers are click-through.
  if (geo.compositeMode && geo.imagesInteractive) {
    for (let i = geo.layerRects.length - 1; i >= 0; i -= 1) {
      const entry = geo.layerRects[i];
      if (!entry) continue;
      const { id, rect } = entry;
      if (geo.buttonsForLayer(id)) {
        const ty = rect.y + BTN_TOP;
        if (Math.hypot(rect.x + rect.w - REMOVE_CX - px, ty - py) < REMOVE_HIT_R)
          return { type: 'layer-remove', id };
        if (Math.hypot(rect.x + rect.w - SNAP_W_CX - px, ty - py) < SNAP_HIT_R)
          return { type: 'layer-snap', id, axis: 'width' };
        if (Math.hypot(rect.x + rect.w - SNAP_H_CX - px, ty - py) < SNAP_HIT_R)
          return { type: 'layer-snap', id, axis: 'height' };
      }
      if (Math.hypot(rect.x + rect.w - px, rect.y + rect.h - py) < RESIZE_HIT_R)
        return { type: 'layer-resize', id };
      if (inRect(px, py, rect)) return { type: 'layer', id };
    }
  }

  for (let i = geo.monitors.length - 1; i >= 0; i -= 1) {
    const m = geo.monitors[i];
    if (!m) continue;
    if (inRect(px, py, m.rect)) return { type: 'monitor', id: m.id };
  }

  if (!geo.compositeMode && geo.imagesInteractive && geo.imageUrl) {
    const r = geo.imgRect;
    if (Math.hypot(r.x + r.w - px, r.y + r.h - py) < IMAGE_RESIZE_HIT_R)
      return { type: 'image-resize' };
    if (inRect(px, py, r)) return { type: 'image' };
  }
  return { type: 'stage' };
}

export function cursorFor(hit: Hit): string {
  switch (hit.type) {
    case 'monitor':
      return 'grab';
    case 'image':
    case 'layer':
      return 'move';
    case 'image-resize':
    case 'layer-resize':
      return 'nwse-resize';
    case 'layer-remove':
    case 'layer-snap':
      return 'pointer';
    default:
      return 'default';
  }
}
