import { describe, it, expect, beforeEach, vi } from 'vitest';

vi.mock('$lib/library/source-image', () => ({
  CANVAS_MAX_EDGE: 1536,
  peekSourceImage: () => null,
  loadSourceImage: (path: string) =>
    Promise.resolve({ url: `data:url/${path}`, naturalW: 1920, naturalH: 1080 }),
}));
vi.mock('$lib/stores/toast.svelte', () => ({ toast: { error: () => {} } }));

import type { Profile } from '$lib/api';
import { appliedCanvas } from './applied.svelte';
import { canvasLayers } from './canvas-layers.svelte';
import { canvasView } from './canvas-view.svelte';
import { imageTransform } from './image-transform.svelte';
import { profileStore } from './profile.svelte';

function profile(): Profile {
  return {
    name: 'p',
    body: {
      type: 'standard',
      layers: [{ path: '/img.png', image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 } }],
    },
    monitor_state: {},
    topology: 'topo-1',
    description: null,
    created_at: '2026-05-10T00:00:00Z',
    updated_at: '2026-05-10T00:00:00Z',
    last_applied_at: null,
    backend_override: null,
  };
}

function moveMonitor(xMm: number): void {
  canvasView.setOverrides({ a: { xMm, yMm: 0 } });
}

beforeEach(() => {
  appliedCanvas.invalidate();
  profileStore.replaceDraft(profile());
  moveMonitor(0);
  canvasLayers.setFromLayers([
    { path: '/img.png', image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 } },
  ]);
  imageTransform.set({ offsetMmX: 0, offsetMmY: 0, widthMm: 1000, heightMm: 600 });
});

describe('appliedCanvas', () => {
  it('unknown_and_un_diverged_before_any_recorded_paint', () => {
    expect(appliedCanvas.known).toBe(false);
    expect(appliedCanvas.diverged).toBe(false);
  });

  it('marking_the_captured_canvas_reads_clean', () => {
    appliedCanvas.mark(appliedCanvas.capture());
    expect(appliedCanvas.known).toBe(true);
    expect(appliedCanvas.diverged).toBe(false);
  });

  it('diverges_when_the_canvas_moves_after_the_mark', () => {
    appliedCanvas.mark(appliedCanvas.capture());
    moveMonitor(50);
    expect(appliedCanvas.diverged).toBe(true);
  });

  // The in-flight-apply race: the baseline must be the canvas that was sent,
  // so an edit made during the round-trip still reads diverged.
  it('mid_apply_edit_stays_diverged_against_the_pre_await_capture', () => {
    const fp = appliedCanvas.capture();
    moveMonitor(50);
    appliedCanvas.mark(fp);
    expect(appliedCanvas.diverged).toBe(true);
  });

  it('follow_moves_the_baseline_with_a_daemon_seed_on_a_clean_canvas', () => {
    appliedCanvas.mark(appliedCanvas.capture());
    appliedCanvas.follow(() => moveMonitor(100));
    expect(appliedCanvas.diverged).toBe(false);
  });

  it('follow_keeps_pending_user_edits_diverged', () => {
    appliedCanvas.mark(appliedCanvas.capture());
    moveMonitor(50);
    appliedCanvas.follow(() => moveMonitor(100));
    expect(appliedCanvas.diverged).toBe(true);
  });

  it('follow_does_not_establish_a_baseline_while_unknown', () => {
    appliedCanvas.follow(() => moveMonitor(100));
    expect(appliedCanvas.known).toBe(false);
  });

  it('invalidate_drops_the_baseline', () => {
    appliedCanvas.mark(appliedCanvas.capture());
    appliedCanvas.invalidate();
    expect(appliedCanvas.known).toBe(false);
    expect(appliedCanvas.diverged).toBe(false);
  });
});
