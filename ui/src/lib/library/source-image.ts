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
  /** Retained size of the `data:` URL. Zero until the load resolves. */
  bytes: number;
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

// Entries are no longer uniformly sized — a 1536px canvas render retains
// several times what a 512px swatch does — so the cache is bounded by retained
// bytes rather than by a count. A count would let a burst of cheap 512px
// inserts (the profile manager opening) evict the expensive canvas renders that
// cost ~155 ms each to rebuild.
export const CACHE_MAX_BYTES = 24 * 1024 * 1024;
/** Floor so one incompressible image can't evict everything behind it. */
export const CACHE_MIN_ENTRIES = 4;
/** Backstop bounding entries still in flight, whose size isn't known yet. */
const MAX_ENTRIES = 32;

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
  const entry: Entry = { promise, loaded: null, bytes: 0 };
  cache.set(key, entry);
  promise.then(
    (img) => {
      entry.loaded = img;
      entry.bytes = img.url.length;
      // Prune again now the real size is known — on insert it counted as zero.
      prune();
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

function retainedBytes(): number {
  let total = 0;
  for (const entry of cache.values()) total += entry.bytes;
  return total;
}

// Evicts oldest-inserted first (Map preserves insertion order).
function prune(): void {
  let bytes = retainedBytes();
  for (const [key, entry] of cache) {
    const overCount = cache.size > MAX_ENTRIES;
    const overBytes = bytes > CACHE_MAX_BYTES && cache.size > CACHE_MIN_ENTRIES;
    if (!overCount && !overBytes) return;
    cache.delete(key);
    bytes -= entry.bytes;
  }
}
