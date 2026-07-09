import { afterEach, describe, expect, it, vi } from 'vitest';
import { api, type RuntimeState } from '$lib/api';
import { toast } from '$lib/stores/toast.svelte';
import { canStepBack, refreshRuntime, slideshowPrev, type SlideshowState } from './actions';

function runtimeState(overrides: Partial<NonNullable<RuntimeState['slideshow']>>): RuntimeState {
  return {
    version: 2,
    active_profile: 'p',
    last_apply_unix_secs: null,
    last_apply_backend: null,
    slideshow: {
      current_index: null,
      history_len: 0,
      paused: false,
      current_path: null,
      remaining_secs: null,
      pool_len: null,
      ...overrides,
    },
  };
}

function slideshowState(historyLen: number): SlideshowState {
  return {
    paused: false,
    index: null,
    total: 0,
    currentPath: null,
    remainingSecs: null,
    historyLen,
    fetchedAt: Date.now(),
  };
}

function clearToasts(): void {
  for (const t of toast.items) toast.dismiss(t.id);
}

afterEach(() => {
  vi.restoreAllMocks();
  clearToasts();
});

describe('refreshRuntime', () => {
  it('carries history_len onto the frontend historyLen field', async () => {
    vi.spyOn(api, 'currentState').mockResolvedValue(runtimeState({ history_len: 3 }));

    const state = await refreshRuntime();

    expect(state?.historyLen).toBe(3);
  });
});

describe('canStepBack', () => {
  it('is false when there is no slideshow state', () => {
    expect(canStepBack(null)).toBe(false);
  });

  it('is false with fewer than two history entries', () => {
    expect(canStepBack(slideshowState(0))).toBe(false);
    expect(canStepBack(slideshowState(1))).toBe(false);
  });

  it('is true once history reaches two entries', () => {
    expect(canStepBack(slideshowState(2))).toBe(true);
    expect(canStepBack(slideshowState(5))).toBe(true);
  });
});

describe('slideshowPrev', () => {
  it('stays silent on the no-history race error', async () => {
    vi.spyOn(api, 'slideshowPrev').mockRejectedValue({
      kind: 'Daemon',
      message: 'no previous image in history',
    });

    await slideshowPrev();

    expect(toast.items).toHaveLength(0);
  });

  it('still toasts other daemon errors', async () => {
    vi.spyOn(api, 'slideshowPrev').mockRejectedValue({
      kind: 'Daemon',
      message: 'something unrelated broke',
    });

    await slideshowPrev();

    expect(toast.items).toHaveLength(1);
    expect(toast.items[0]?.title).toBe('Slideshow prev failed');
  });
});
