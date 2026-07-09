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

// Entries are not uniformly sized — a 1536px canvas render retains several
// times what a 512px swatch does — so two bounds apply and whichever binds
// first wins.
//
// In the ordinary case the count is the operative limit: 32 typical canvas
// renders (~683 KiB of base64 each) come to ~21 MiB, inside the byte budget.
// The byte budget is the guard against pathological entries — 32 incompressible
// 2048px sources would otherwise retain ~137 MiB.
/** Ceiling on retained `data:` URL bytes. Guards against outsized entries. */
export const CACHE_MAX_BYTES = 24 * 1024 * 1024;
/** The ordinary limit, and a bound on in-flight entries whose size isn't known yet. */
const MAX_ENTRIES = 32;
/**
 * Floor so a few outsized images can't evict everything behind them. Sized to
 * hold a whole canvas: layer count is unbounded, but a profile with more than
 * this many layers can still thrash under byte pressure.
 */
export const CACHE_MIN_ENTRIES = 8;

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
  if (existing) {
    touch(key, existing);
    return existing.promise;
  }
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
      // Only evict our own entry. `prune()` can drop an in-flight entry (its
      // `bytes` are zero until it settles), after which a re-request installs
      // a fresh one under the same key — and this rejection must not take the
      // replacement down with it.
      if (cache.get(key) === entry) cache.delete(key);
    },
  );
  prune();
  return promise;
}

export function peekSourceImage(
  path: string,
  maxEdge: number = PREVIEW_MAX_EDGE,
): SourceImage | null {
  const key = keyFor(path, maxEdge);
  const entry = cache.get(key);
  if (!entry) return null;
  touch(key, entry);
  return entry.loaded;
}

// Re-insert at the tail so `prune()` sees least-recently-*used* order rather
// than insertion order. The canvas peeks its layers on every re-render, so this
// keeps them ahead of swatches loaded once and never read again.
function touch(key: string, entry: Entry): void {
  cache.delete(key);
  cache.set(key, entry);
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

// Evicts least-recently-used first (`touch` keeps Map order = LRU order).
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
