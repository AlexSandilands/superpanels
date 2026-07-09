// Tracks the canvas state last known to be painted on the desktop, so the
// Apply button can fade once the desktop matches the canvas. This is a
// different baseline from the profile store's `dirty` flag: Apply paints
// without saving, Save persists without painting, and either one alone can
// be clean.

import { canvasFingerprint } from '$lib/canvas/dirty';
import { canvasView } from './canvas-view.svelte';
import { canvasLayers } from './canvas-layers.svelte';
// Cyclic with image-transform (its daemon-driven seeds call `follow`) —
// safe: both modules only touch the other inside functions.
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
  /** `false` until a paint is recorded (or after `invalidate`) — callers
   *  fall back to their own dirty signal while the desktop is unknown. */
  get known(): boolean {
    return applied !== null;
  },

  /** `true` when the canvas no longer matches the last recorded paint.
   *  `false` while nothing is known — gate on `known` first. */
  get diverged(): boolean {
    return applied !== null && applied !== live();
  },

  /** Fingerprint of the live canvas. Capture next to the payload an apply
   *  sends and `mark` the same value on success — capturing after the
   *  round-trip would baseline edits made while the apply was in flight. */
  capture(): string {
    return live();
  },

  /** Record `fp` (from `capture`) as what the desktop now shows. */
  mark(fp: string): void {
    applied = fp;
  },

  /** Run a canvas write that mirrors state the daemon painted on its own
   *  (slideshow-advance reseed). The baseline moves with the write only when
   *  the canvas matched it going in — pending user edits keep Apply lit. */
  follow(seed: () => void): void {
    const clean = applied !== null && applied === live();
    seed();
    if (clean) applied = live();
  },

  /** The desktop changed behind our back (schedule fire over a dirty
   *  canvas) — drop the baseline rather than trust a stale one. */
  invalidate(): void {
    applied = null;
  },
};
