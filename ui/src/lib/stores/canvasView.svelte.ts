// Canvas-only view state: zoom, pan, dim, hover/selection, and per-monitor
// preview-only position+rotation overrides. The detected layout from
// `monitorStore` is the source of truth; overrides let the user rearrange
// monitors visually without (yet) pushing back to the compositor.

export type MonitorOverride = {
  xMm: number;
  yMm: number;
  rotation: 0 | 90 | 180 | 270;
};

let zoom = $state(1);
let panX = $state(0);
let panY = $state(0);
let dim = $state(false);
let hoverId = $state<string | null>(null);
let selectId = $state<string | null>(null);
let overrides = $state<Record<string, MonitorOverride>>({});

export const canvasView = {
  get zoom() {
    return zoom;
  },
  setZoom(z: number) {
    zoom = Math.max(0.5, Math.min(2.0, z));
  },
  get panX() {
    return panX;
  },
  get panY() {
    return panY;
  },
  setPan(x: number, y: number) {
    panX = x;
    panY = y;
  },
  resetPan() {
    panX = 0;
    panY = 0;
  },
  get dim() {
    return dim;
  },
  setDim(v: boolean) {
    dim = v;
  },
  toggleDim() {
    dim = !dim;
  },
  get hoverId() {
    return hoverId;
  },
  setHoverId(id: string | null) {
    hoverId = id;
  },
  get selectId() {
    return selectId;
  },
  setSelectId(id: string | null) {
    selectId = id;
  },

  get overrides() {
    return overrides;
  },
  override(id: string, patch: Partial<MonitorOverride>) {
    const existing = overrides[id];
    if (!existing) return;
    overrides[id] = { ...existing, ...patch };
  },
  setOverrides(next: Record<string, MonitorOverride>) {
    overrides = next;
  },
  resetOverrides(defaults: Record<string, MonitorOverride>) {
    overrides = { ...defaults };
  },
  hasOverrides(defaults: Record<string, MonitorOverride>): boolean {
    for (const id of Object.keys(defaults)) {
      const a = defaults[id];
      const b = overrides[id];
      if (!a || !b) continue;
      if (
        Math.abs(a.xMm - b.xMm) > 0.5 ||
        Math.abs(a.yMm - b.yMm) > 0.5 ||
        a.rotation !== b.rotation
      ) {
        return true;
      }
    }
    return false;
  },
};
