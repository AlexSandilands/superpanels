// Quick-jump grid images for the active slideshow set. Prefers the daemon's
// resolved pool (`slideshow_pool`) — folder sources can include paths outside
// the library index, and the daemon owns the recursive/sort/filter rules for
// resolving them, so the frontend must not re-walk folders itself. Falls back
// to the on-disk library intersection only when the daemon is unreachable.

import { api, errorMessage, isIpcError, type LibraryEntry } from '$lib/api';
import { membershipLookup } from '$lib/slideshow-set';
import { toast } from '$lib/stores/toast.svelte';
import type { SlideshowSource } from '$lib/types/profile-helpers';

let images = $state<string[]>([]);

export const slideshowJumpImages = {
  get value() {
    return images;
  },
};

/** Library-indexed subset of `source` — the pre-daemon-pool behaviour, used
 *  only while the daemon socket itself is unreachable. */
export function libraryIntersection(source: SlideshowSource, entries: LibraryEntry[]): string[] {
  const member = membershipLookup(source.images);
  return entries
    .filter((e) => member(e.path) !== null)
    .map((e) => e.path)
    .sort((a, b) => a.localeCompare(b));
}

/** Keeps `slideshowJumpImages.value` in step with the active slideshow
 *  target. Call once during component setup (mirrors `useSourceImage` in
 *  `image-transform.svelte.ts`). */
export function useSlideshowJumpImages(
  getProfileName: () => string | null,
  getSource: () => SlideshowSource | null,
  getLibraryEntries: () => LibraryEntry[],
): void {
  $effect(() => {
    const name = getProfileName();
    const source = getSource();
    const entries = getLibraryEntries();
    if (!name || !source) {
      images = [];
      return;
    }
    let cancelled = false;
    void api
      .slideshowPool(name)
      .then((pool) => {
        if (cancelled) return;
        images = [...pool].sort((a, b) => a.localeCompare(b));
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        if (isIpcError(err) && err.kind === 'DaemonUnreachable') {
          images = libraryIntersection(source, entries);
          return;
        }
        toast.error('Could not load slideshow images', errorMessage(err));
        images = [];
      });
    return () => {
      cancelled = true;
    };
  });
}
