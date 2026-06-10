// Pure helpers for editing a slideshow ImageSet. All functions return new
// objects — callers persist the result via `update_profile_source`.

import type { ImageOverride } from '$lib/types/ImageOverride';
import type { ImageSet } from '$lib/types/ImageSet';
import type { ImageSource } from '$lib/types/ImageSource';

export type Membership = 'image' | 'folder' | null;

export function emptyImageSet(): ImageSet {
  return { sources: [] };
}

/** How `path` is covered by the set: as a hand-picked image, via a folder
 *  source, or not at all. Direct membership wins so removal stays possible. */
export function membership(set: ImageSet, path: string): Membership {
  return membershipLookup(set)(path);
}

/** Precomputed membership lookup — O(1) for picked images, O(folders) for
 *  folder coverage. Build it once per set change; grids query per card. */
export function membershipLookup(set: ImageSet): (path: string) => Membership {
  const imagePaths = new Set<string>();
  const folders: { path: string; recursive: boolean }[] = [];
  for (const s of set.sources) {
    if (s.type === 'image') imagePaths.add(s.path);
    else folders.push(s);
  }
  return (path) => {
    if (imagePaths.has(path)) return 'image';
    return folders.some((f) => underFolder(path, f.path, f.recursive)) ? 'folder' : null;
  };
}

export function addImage(set: ImageSet, path: string): ImageSet {
  if (set.sources.some((s) => s.type === 'image' && s.path === path)) return set;
  return { sources: [...set.sources, { type: 'image', path }] };
}

export function removeImage(set: ImageSet, path: string): ImageSet {
  return { sources: set.sources.filter((s) => !(s.type === 'image' && s.path === path)) };
}

export function addFolder(set: ImageSet, path: string, recursive: boolean): ImageSet {
  if (set.sources.some((s) => s.type === 'folder' && s.path === path)) return set;
  return { sources: [...set.sources, { type: 'folder', path, recursive }] };
}

export function removeSourceAt(set: ImageSet, index: number): ImageSet {
  return { sources: set.sources.filter((_, i) => i !== index) };
}

/** Drop per-image overrides whose image is no longer covered by `set`
 *  (hand-picked image or folder source removed). Folder-covered paths
 *  survive, so a tweak inside a kept folder is preserved. */
export function gcOverrides(
  overrides: { [key in string]: ImageOverride } | undefined,
  set: ImageSet,
): { [key in string]: ImageOverride } {
  if (!overrides) return {};
  const member = membershipLookup(set);
  return Object.fromEntries(Object.entries(overrides).filter(([path]) => member(path) !== null));
}

// Relative aspect-ratio slack treated as "the same shape" — covers rounding
// from odd pixel counts without masking a real 16:9 vs 21:9 mismatch.
const ASPECT_TOLERANCE = 0.02;

export type AspectMismatch = { mismatched: number; known: number };

/** Of the library entries covered by `set`, how many would be distorted by a
 *  uniform layout with `rectAspect`? Dimension data comes from the library
 *  index, so images outside it aren't counted (`known` says how many were). */
export function countAspectMismatches(
  entries: { path: string; aspect_ratio: number }[],
  set: ImageSet,
  rectAspect: number,
): AspectMismatch {
  const member = membershipLookup(set);
  let known = 0;
  let mismatched = 0;
  if (!Number.isFinite(rectAspect) || rectAspect <= 0) return { mismatched, known };
  for (const e of entries) {
    if (member(e.path) === null) continue;
    known += 1;
    if (Math.abs(e.aspect_ratio - rectAspect) / rectAspect > ASPECT_TOLERANCE) mismatched += 1;
  }
  return { mismatched, known };
}

export function sourceLabel(source: ImageSource): string {
  const name = baseName(source.path);
  return source.type === 'folder' ? `${name}/` : name;
}

export function imageCount(set: ImageSet): number {
  return set.sources.filter((s) => s.type === 'image').length;
}

export function folderCount(set: ImageSet): number {
  return set.sources.filter((s) => s.type === 'folder').length;
}

function baseName(path: string): string {
  const trimmed = path.endsWith('/') ? path.slice(0, -1) : path;
  const i = trimmed.lastIndexOf('/');
  return i >= 0 ? trimmed.slice(i + 1) : trimmed;
}

function underFolder(path: string, folder: string, recursive: boolean): boolean {
  // Component-wise compare, mirroring the daemon's `Path::strip_prefix` in
  // pool.rs — raw string prefixes disagree on trailing slashes and `//`.
  const folderParts = folder.split('/').filter(Boolean);
  const pathParts = path.split('/').filter(Boolean);
  if (pathParts.length <= folderParts.length) return false;
  if (!folderParts.every((part, i) => part === pathParts[i])) return false;
  return recursive || pathParts.length === folderParts.length + 1;
}
