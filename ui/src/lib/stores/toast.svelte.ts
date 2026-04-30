// Non-blocking toast queue (`SPEC.md` §12.7). Errors stay until dismissed;
// successes auto-fade after a few seconds. Expiry is driven by an `$effect`
// in `Toast.svelte` so we don't leak ad-hoc timers from a module scope.

export type Severity = 'success' | 'error' | 'info';

export type Toast = {
  id: number;
  severity: Severity;
  title: string;
  detail?: string;
  expiresAt?: number;
};

let next = 0;
let items = $state<Toast[]>([]);

const DEFAULT_TIMEOUT_MS = 4000;

function push(severity: Severity, title: string, detail?: string, timeoutMs?: number): number {
  const id = ++next;
  const expiresAt =
    severity === 'error' ? undefined : Date.now() + (timeoutMs ?? DEFAULT_TIMEOUT_MS);
  const t: Toast = {
    id,
    severity,
    title,
    ...(detail !== undefined ? { detail } : {}),
    ...(expiresAt !== undefined ? { expiresAt } : {}),
  };
  items = [...items, t];
  return id;
}

function dismiss(id: number) {
  items = items.filter((t) => t.id !== id);
}

export const toast = {
  get items() {
    return items;
  },
  success: (title: string, detail?: string) => push('success', title, detail),
  error: (title: string, detail?: string) => push('error', title, detail),
  info: (title: string, detail?: string) => push('info', title, detail),
  dismiss,
};
