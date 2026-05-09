// Slideshow controller — owns the local mirror of the daemon's slideshow
// state (paused / current index / total) and the next / prev / toggle-pause
// orchestration. Exposed as a module-scoped singleton so any component can
// trigger transport without prop-drilling.

import {
  refreshRuntime,
  slideshowNext as runNext,
  slideshowPrev as runPrev,
  slideshowTogglePause as runTogglePause,
  type SlideshowState,
} from '$lib/actions';

let state = $state<SlideshowState>(null);

async function refresh(): Promise<void> {
  const next = await refreshRuntime();
  if (next !== undefined) state = next;
}

export const slideshowController = {
  get state() {
    return state;
  },
  refresh,
  async next() {
    await runNext();
    await refresh();
  },
  async prev() {
    await runPrev();
    await refresh();
  },
  async togglePause() {
    const r = await runTogglePause();
    if (r) state = state ? { ...state, paused: r.paused } : null;
  },
};
