// Representative image for a profile's switcher / tray thumbnail.

import type { Profile } from '$lib/api';
import { membershipLookup } from '$lib/slideshow-set';

/** The image that stands for `p` in compact profile lists: a standard
 *  profile's top layer; for slideshows the first hand-picked image, else the
 *  alphabetically first library entry covered by a folder source
 *  (`libraryPaths` comes from the library index — folder contents aren't
 *  enumerated anywhere else client-side). Per-monitor profiles and empty sets
 *  have no thumbnail. */
export function profileThumbPath(p: Profile, libraryPaths: readonly string[]): string | null {
  if (p.body.type === 'standard') {
    const top = p.body.layers.at(-1);
    return top ? validPath(top.path) : null;
  }
  if (p.body.type !== 'slideshow') return null;
  const src = p.body.source;
  const picked = src.images.sources.find((s) => s.type === 'image');
  if (picked) return validPath(picked.path);
  const member = membershipLookup(src.images);
  let best: string | null = null;
  for (const path of libraryPaths) {
    if (best !== null && path >= best) continue;
    if (member(path) !== null) best = path;
  }
  return best ? validPath(best) : null;
}

// Thumbnail fetches go to the daemon by path — only absolute paths are valid.
function validPath(path: string): string | null {
  const trimmed = path.trim();
  return trimmed.startsWith('/') ? trimmed : null;
}
