// Runtime info exposed in the title bar status pill: last-applied backend,
// elapsed time, "Xs ago" relative timestamp, and an apply-flash key the
// canvas listens to.

export type ApplyMeta = {
  backend: string;
  elapsedMs: number;
  monitorsSet: number;
  at: number;
};

let last = $state<ApplyMeta | null>(null);
let flashKey = $state(0);
let flashAt = $state(0);

function formatRelative(ms: number): string {
  if (ms < 1500) return 'just now';
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  return `${h}h ago`;
}

export const runtime = {
  get last() {
    return last;
  },
  get flashKey() {
    return flashKey;
  },
  get flashAt() {
    return flashAt;
  },
  recordApply(meta: ApplyMeta) {
    last = meta;
    flashKey += 1;
    flashAt = meta.at;
  },
  describeLastApply(now: number = Date.now()): string {
    return last ? formatRelative(now - last.at) : '—';
  },
};
