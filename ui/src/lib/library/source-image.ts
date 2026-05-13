// Source-image cache for the preview canvas. Returns a `data:` URL plus the
// natural pixel size so the canvas can reason about aspect ratio. Backed by
// the `source_thumbnail` IPC command which renders any path,
// not just library-rooted files.

import { api } from '$lib/api';

export type SourceImage = {
  url: string;
  naturalW: number;
  naturalH: number;
};

type Entry = {
  promise: Promise<SourceImage>;
  loaded: SourceImage | null;
};

const MAX_ENTRIES = 16;
const cache = new Map<string, Entry>();

export async function loadSourceImage(path: string): Promise<SourceImage> {
  const existing = cache.get(path);
  if (existing) return existing.promise;
  const promise = fetchAndDecode(path);
  const entry: Entry = { promise, loaded: null };
  cache.set(path, entry);
  promise.then(
    (img) => {
      entry.loaded = img;
    },
    () => {
      cache.delete(path);
    },
  );
  prune();
  return promise;
}

export function peekSourceImage(path: string): SourceImage | null {
  return cache.get(path)?.loaded ?? null;
}

async function fetchAndDecode(path: string): Promise<SourceImage> {
  const { data, mime } = await api.sourceThumbnail(path);
  const url = `data:${mime};base64,${data}`;
  return await new Promise((resolve, reject) => {
    const img = new Image();
    img.decoding = 'async';
    img.onload = () => {
      resolve({ url, naturalW: img.naturalWidth, naturalH: img.naturalHeight });
    };
    img.onerror = () =>
      reject(new Error(`failed to decode source thumbnail for ${path} (mime=${mime})`));
    img.src = url;
  });
}

function prune(): void {
  if (cache.size <= MAX_ENTRIES) return;
  const overflow = cache.size - MAX_ENTRIES;
  let dropped = 0;
  for (const key of cache.keys()) {
    if (dropped >= overflow) break;
    cache.delete(key);
    dropped += 1;
  }
}
