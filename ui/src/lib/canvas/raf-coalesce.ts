// Collapse a burst of samples into at most one delivery per animation frame.
// High-polling-rate pointers fire well above display rate; the canvas only
// needs the newest sample each frame. The scheduler is injectable so the
// coalescing logic is unit-testable without a live `requestAnimationFrame`.

export type RafScheduler = {
  request: (cb: () => void) => number;
  cancel: (handle: number) => void;
};

const defaultScheduler: RafScheduler = {
  request: (cb) =>
    typeof requestAnimationFrame === 'function' ? requestAnimationFrame(() => cb()) : 0,
  cancel: (handle) => {
    if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(handle);
  },
};

export type Coalescer<T> = {
  /** Record the newest sample; schedule a frame if one isn't already pending. */
  push: (sample: T) => void;
  /** Deliver the pending sample now and drop the scheduled frame. */
  flush: () => void;
  /** Drop the pending sample and the scheduled frame without delivering. */
  cancel: () => void;
  readonly hasPending: boolean;
};

export function createRafCoalescer<T>(
  onFrame: (sample: T) => void,
  scheduler: RafScheduler = defaultScheduler,
): Coalescer<T> {
  // Boxed so a legitimately-`undefined` sample is distinguishable from "empty".
  let pending: { value: T } | null = null;
  let handle: number | null = null;

  function deliver() {
    if (!pending) return;
    const s = pending.value;
    pending = null;
    onFrame(s);
  }

  function tick() {
    handle = null;
    deliver();
  }

  return {
    push(sample) {
      pending = { value: sample };
      if (handle === null) handle = scheduler.request(tick);
    },
    flush() {
      if (handle !== null) {
        scheduler.cancel(handle);
        handle = null;
      }
      deliver();
    },
    cancel() {
      if (handle !== null) {
        scheduler.cancel(handle);
        handle = null;
      }
      pending = null;
    },
    get hasPending() {
      return pending !== null;
    },
  };
}
