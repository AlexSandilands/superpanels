// Lazy source-image thumbnail loader. Calls the dedicated `source_thumbnail`
// IPC command (`SPEC §12.4`), which renders a resized PNG locally regardless
// of library-roots configuration — the canvas preview must work for any
// selected/dropped file, including paths outside the user's library roots.

import { api } from '$lib/api';

type CacheEntry = {
  promise: Promise<HTMLImageElement>;
  image: HTMLImageElement | null;
};

const cache = new Map<string, CacheEntry>();
const MAX_ENTRIES = 16;

export async function loadThumbnail(path: string): Promise<HTMLImageElement> {
  const existing = cache.get(path);
  if (existing) return existing.promise;
  const promise = fetchAndDecode(path);
  const entry: CacheEntry = { promise, image: null };
  cache.set(path, entry);
  // Update the cache when the promise settles. Caller (`MonitorCanvas`)
  // is the one wired to surface failures via `onImageLoadError`; we just
  // drop the failed entry so a retry re-fetches. The `.catch(noop)` keeps
  // this branch from flagging an unhandled rejection — the original
  // `promise` is still returned, so the caller's `await` sees the error.
  promise.then(
    (img) => {
      entry.image = img;
    },
    () => {
      cache.delete(path);
    },
  );
  pruneCache();
  return promise;
}

export function peekThumbnail(path: string): HTMLImageElement | null {
  return cache.get(path)?.image ?? null;
}

async function fetchAndDecode(path: string): Promise<HTMLImageElement> {
  const { data, mime } = await api.sourceThumbnail(path);
  const url = `data:${mime};base64,${data}`;
  return await new Promise((resolve, reject) => {
    const img = new Image();
    img.decoding = 'async';
    img.onload = () => resolve(img);
    img.onerror = () =>
      reject(new Error(`failed to decode thumbnail bytes for ${path} (mime=${mime})`));
    img.src = url;
  });
}

function pruneCache(): void {
  if (cache.size <= MAX_ENTRIES) return;
  // Map iteration is insertion-ordered; drop the oldest until we're back
  // under the budget. The thumbnail size is capped at 320 px so this is a
  // small memory footprint either way.
  const overflow = cache.size - MAX_ENTRIES;
  let dropped = 0;
  for (const key of cache.keys()) {
    if (dropped >= overflow) break;
    cache.delete(key);
    dropped += 1;
  }
}
