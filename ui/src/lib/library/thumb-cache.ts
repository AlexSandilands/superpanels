// Library-grid thumbnail cache. Calls `library_thumbnail` and stores the
// decoded bytes as a Blob → object URL so the grid binds an `<img src>`
// without re-base64-decoding on every reflow. Bounded LRU (oldest-evicted)
// per `SPEC §7.2` (thumb cache budget).

import { api } from '$lib/api';

type Entry = {
  promise: Promise<string>;
  url: string | null;
};

const MAX_ENTRIES = 256;

const cache = new Map<string, Entry>();

export async function getLibraryThumbUrl(path: string): Promise<string> {
  const existing = cache.get(path);
  if (existing) {
    cache.delete(path);
    cache.set(path, existing);
    return existing.promise;
  }
  const promise = fetchAsObjectUrl(path);
  const entry: Entry = { promise, url: null };
  cache.set(path, entry);
  promise.then(
    (url) => {
      entry.url = url;
    },
    () => {
      cache.delete(path);
    },
  );
  evictOverflow();
  return promise;
}

export function dropLibraryThumb(path: string): void {
  const entry = cache.get(path);
  if (!entry) return;
  if (entry.url) URL.revokeObjectURL(entry.url);
  cache.delete(path);
}

async function fetchAsObjectUrl(path: string): Promise<string> {
  const { data, mime } = await api.libraryThumbnail(path);
  const bin = atob(data);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  const blob = new Blob([bytes], { type: mime });
  return URL.createObjectURL(blob);
}

function evictOverflow(): void {
  if (cache.size <= MAX_ENTRIES) return;
  const overflow = cache.size - MAX_ENTRIES;
  let dropped = 0;
  for (const key of cache.keys()) {
    if (dropped >= overflow) break;
    const entry = cache.get(key);
    if (entry?.url) URL.revokeObjectURL(entry.url);
    cache.delete(key);
    dropped += 1;
  }
}
