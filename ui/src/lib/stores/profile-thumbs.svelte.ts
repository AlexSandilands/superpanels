// Shared thumbnail cache for the profile switcher and tray menu, keyed by
// image path so a profile's source edits pick up a fresh thumb naturally.

import { api, type Profile } from '$lib/api';
import { profileThumbPath } from '$lib/profile-thumb';

// '' marks a failed fetch — render the swatch fallback without retrying on
// every menu open.
const thumbs = $state<Record<string, string>>({});
const pending = new Set<string>();

export const profileThumbs = {
  /** Data-URL thumbnail for `path`; kicks off the fetch on first miss and
   *  returns `null` until it lands (the read subscribes the caller). The
   *  daemon's mtime-keyed cache is tried first — instant on repeat sessions;
   *  paths outside the library roots fall back to a local render. */
  url(path: string | null): string | null {
    if (!path) return null;
    const hit = thumbs[path];
    if (hit !== undefined) return hit || null;
    if (!pending.has(path)) {
      pending.add(path);
      void api
        .libraryThumbnail(path)
        .catch(() => api.sourceThumbnail(path))
        .then((r) => {
          thumbs[path] = `data:${r.mime};base64,${r.data}`;
        })
        .catch(() => {
          thumbs[path] = '';
        })
        .finally(() => pending.delete(path));
    }
    return null;
  },
};

/** Kick off fetches for every profile's thumbnail without waiting for a menu
 *  to open — by the first click they're already cached. Idempotent. */
export function prewarmProfileThumbs(profiles: Profile[], libraryPaths: readonly string[]): void {
  for (const p of profiles) {
    profileThumbs.url(profileThumbPath(p, libraryPaths));
  }
}
