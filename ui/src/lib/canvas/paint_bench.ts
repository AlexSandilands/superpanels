// SPEC §19 "Canvas drag → redraw frame" baseline-capture hook. Toggle via
// `localStorage.setItem('superpanels.bench', '1')` in the webview console;
// frame timings land on `window.__superpanelsPaint` (last 240 samples).

const HISTORY_LIMIT = 240;

export function paintInstrumentationEnabled(): boolean {
  return typeof window !== 'undefined' && window.localStorage?.getItem('superpanels.bench') === '1';
}

export function recordPaint(ms: number): void {
  if (typeof window === 'undefined') return;
  const w = window as Window & { __superpanelsPaint?: number[] };
  if (!w.__superpanelsPaint) w.__superpanelsPaint = [];
  w.__superpanelsPaint.push(ms);
  if (w.__superpanelsPaint.length > HISTORY_LIMIT) w.__superpanelsPaint.shift();
}
