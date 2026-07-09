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

/** Small previews — profile swatches and the profile-detail panel. Matches
 *  `LibraryConfig::thumbnail_size`'s default. */
export const PREVIEW_MAX_EDGE = 512;

/** The preview canvas stretches one source image across the whole desktop
 *  plane, so a grid-sized thumbnail reads as soft and blocky. 1536px is the
 *  point where a spanned image looks sharp without the decode (~130 ms) or the
 *  base64 `data:` URL (~0.6 MiB for a photo) becoming noticeable; the Rust side
 *  caps any request at 2048. */
export const CANVAS_MAX_EDGE = 1536;

const MAX_ENTRIES = 16;
const cache = new Map<string, Entry>();

// Keyed by edge as well as path: the canvas and the profile modal ask for the
// same file at different sizes and must not serve each other's copy.
function keyFor(path: string, maxEdge: number): string {
  return `${maxEdge}:${path}`;
}

export async function loadSourceImage(
  path: string,
  maxEdge: number = PREVIEW_MAX_EDGE,
): Promise<SourceImage> {
  const key = keyFor(path, maxEdge);
  const existing = cache.get(key);
  if (existing) return existing.promise;
  const promise = fetchAndDecode(path, maxEdge);
  const entry: Entry = { promise, loaded: null };
  cache.set(key, entry);
  promise.then(
    (img) => {
      entry.loaded = img;
    },
    () => {
      cache.delete(key);
    },
  );
  prune();
  return promise;
}

export function peekSourceImage(
  path: string,
  maxEdge: number = PREVIEW_MAX_EDGE,
): SourceImage | null {
  return cache.get(keyFor(path, maxEdge))?.loaded ?? null;
}

async function fetchAndDecode(path: string, maxEdge: number): Promise<SourceImage> {
  const { data, mime } = await api.sourceThumbnail(path, maxEdge);
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
