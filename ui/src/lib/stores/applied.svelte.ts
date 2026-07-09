// Tracks the canvas state that was last pushed to the desktop, so the Apply
// button can fade once the desktop matches the canvas. This is a different
// baseline from the profile store's `dirty` flag: Apply paints without saving,
// Save persists without painting, and either one alone can be clean.

import { canvasFingerprint } from '$lib/canvas/dirty';
import { canvasView } from './canvas-view.svelte';
import { canvasLayers } from './canvas-layers.svelte';
import { imageTransform } from './image-transform.svelte';
import { profileStore } from './profile.svelte';

let applied = $state<string | null>(null);

function live(): string {
  return canvasFingerprint(
    profileStore.draft,
    canvasView.overrides,
    canvasLayers.list,
    imageTransform.value,
  );
}

export const appliedCanvas = {
  /** `true` when the canvas no longer matches the last Apply — including
   *  before the first Apply of the session, when nothing is known. */
  get diverged(): boolean {
    return applied === null || applied !== live();
  },

  /** Record the canvas as applied. Call only after the backend confirms. */
  mark(): void {
    applied = live();
  },
};
