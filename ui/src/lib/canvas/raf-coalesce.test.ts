import { describe, it, expect, vi } from 'vitest';
import { createRafCoalescer, type RafScheduler } from './raf-coalesce';

/** A hand-driven scheduler: `run()` fires the single pending frame callback. */
function manualScheduler() {
  let next = 1;
  const cbs = new Map<number, () => void>();
  const scheduler: RafScheduler = {
    request(cb) {
      const id = next++;
      cbs.set(id, cb);
      return id;
    },
    cancel(id) {
      cbs.delete(id);
    },
  };
  return {
    scheduler,
    run() {
      const pending = [...cbs.entries()];
      cbs.clear();
      for (const [, cb] of pending) cb();
    },
    get scheduled() {
      return cbs.size;
    },
  };
}

describe('createRafCoalescer', () => {
  it('collapses many pushes in a frame to one delivery of the newest sample', () => {
    const m = manualScheduler();
    const onFrame = vi.fn<(n: number) => void>();
    const c = createRafCoalescer(onFrame, m.scheduler);

    c.push(1);
    c.push(2);
    c.push(3);
    expect(onFrame).not.toHaveBeenCalled();
    expect(m.scheduled).toBe(1); // only one frame requested for the burst

    m.run();
    expect(onFrame).toHaveBeenCalledTimes(1);
    expect(onFrame).toHaveBeenCalledWith(3);
  });

  it('requests a fresh frame for a push that arrives after the previous frame ran', () => {
    const m = manualScheduler();
    const onFrame = vi.fn<(n: number) => void>();
    const c = createRafCoalescer(onFrame, m.scheduler);

    c.push(1);
    m.run();
    c.push(2);
    m.run();

    expect(onFrame.mock.calls).toEqual([[1], [2]]);
  });

  it('flush delivers the pending sample immediately and drops the frame', () => {
    const m = manualScheduler();
    const onFrame = vi.fn<(n: number) => void>();
    const c = createRafCoalescer(onFrame, m.scheduler);

    c.push(7);
    c.flush();
    expect(onFrame).toHaveBeenCalledExactlyOnceWith(7);
    expect(m.scheduled).toBe(0);

    // The dropped frame must not double-deliver if it somehow fires later.
    m.run();
    expect(onFrame).toHaveBeenCalledTimes(1);
  });

  it('flush with nothing pending is a no-op', () => {
    const m = manualScheduler();
    const onFrame = vi.fn<(n: number) => void>();
    const c = createRafCoalescer(onFrame, m.scheduler);

    c.flush();
    expect(onFrame).not.toHaveBeenCalled();
  });

  it('cancel drops the pending sample without delivering', () => {
    const m = manualScheduler();
    const onFrame = vi.fn<(n: number) => void>();
    const c = createRafCoalescer(onFrame, m.scheduler);

    c.push(9);
    expect(c.hasPending).toBe(true);
    c.cancel();
    expect(c.hasPending).toBe(false);
    m.run();
    expect(onFrame).not.toHaveBeenCalled();
  });
});
