// Canvas-only view state: zoom, pan, dim, hover/selection, and per-monitor
// preview-only position overrides. The detected layout from `monitorStore`
// is the source of truth (rotation included — that comes from the OS and is
// never user-authored). Overrides let the user rearrange monitors visually
// without (yet) pushing back to the compositor.

export type MonitorOverride = {
  xMm: number;
  yMm: number;
};

/** Which class of object pointer gestures target. Images win by default; the
 *  user flips to `monitors` to rearrange the layout without the layers
 *  intercepting drags. */
export type CanvasMode = 'images' | 'monitors';

let zoom = $state(1);
let panX = $state(0);
let panY = $state(0);
// On by default: the off-monitor dim makes the live crop read at a glance.
let dim = $state(true);
let hoverId = $state<string | null>(null);
let selectId = $state<string | null>(null);
let selectedLayerId = $state<string | null>(null);
let mode = $state<CanvasMode>('images');
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
  get selectedLayerId() {
    return selectedLayerId;
  },
  setSelectedLayerId(id: string | null) {
    selectedLayerId = id;
  },
  get mode() {
    return mode;
  },
  setMode(m: CanvasMode) {
    mode = m;
  },
  toggleMode() {
    mode = mode === 'images' ? 'monitors' : 'images';
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
      if (Math.abs(a.xMm - b.xMm) > 0.5 || Math.abs(a.yMm - b.yMm) > 0.5) {
        return true;
      }
    }
    return false;
  },
};
