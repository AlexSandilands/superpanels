// Non-blocking toast queue (`SPEC.md` §12.7). Errors stay until dismissed;
// successes auto-fade after a few seconds. Expiry is driven by an `$effect`
// in `Toast.svelte` so we don't leak ad-hoc timers from a module scope.

export type Severity = 'success' | 'error' | 'info';

export type ToastAction = {
  label: string;
  onClick: () => void;
};

export type Toast = {
  id: number;
  severity: Severity;
  title: string;
  detail?: string;
  expiresAt?: number;
  action?: ToastAction;
};

export type ToastOptions = {
  detail?: string;
  action?: ToastAction;
  timeoutMs?: number;
};

let next = 0;
let items = $state<Toast[]>([]);

const DEFAULT_TIMEOUT_MS = 4000;
const ACTION_TIMEOUT_MS = 8000;

function push(severity: Severity, title: string, opts?: ToastOptions): number {
  const id = ++next;
  const baseTimeout = opts?.timeoutMs ?? (opts?.action ? ACTION_TIMEOUT_MS : DEFAULT_TIMEOUT_MS);
  const expiresAt = severity === 'error' ? undefined : Date.now() + baseTimeout;
  const t: Toast = {
    id,
    severity,
    title,
    ...(opts?.detail !== undefined ? { detail: opts.detail } : {}),
    ...(opts?.action !== undefined ? { action: opts.action } : {}),
    ...(expiresAt !== undefined ? { expiresAt } : {}),
  };
  items = [...items, t];
  return id;
}

function dismiss(id: number) {
  items = items.filter((t) => t.id !== id);
}

function normalise(detailOrOpts?: string | ToastOptions): ToastOptions | undefined {
  if (detailOrOpts === undefined) return undefined;
  if (typeof detailOrOpts === 'string') return { detail: detailOrOpts };
  return detailOrOpts;
}

export const toast = {
  get items() {
    return items;
  },
  success: (title: string, detailOrOpts?: string | ToastOptions) =>
    push('success', title, normalise(detailOrOpts)),
  error: (title: string, detailOrOpts?: string | ToastOptions) =>
    push('error', title, normalise(detailOrOpts)),
  info: (title: string, detailOrOpts?: string | ToastOptions) =>
    push('info', title, normalise(detailOrOpts)),
  dismiss,
};
