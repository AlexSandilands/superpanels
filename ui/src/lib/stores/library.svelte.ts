// Library store. Phase 4b: typed entries, client-side filtering, sort, and
// search. The daemon paginates `library_list` server-side; we walk the pages
// and concatenate so the IPC frame cap (1 MiB per response, see
// `superpanels-daemon/src/server/frame.rs::MAX_FRAME_BYTES`) is respected for
// libraries of any size, then slice locally so filter changes don't spend an
// IPC roundtrip per keystroke.

import { api, errorMessage, type LibraryEntry } from '$lib/api';
import { getLibraryThumbUrl } from '$lib/library/thumb-cache';
import type { LibraryFilter } from '$lib/types/LibraryFilter';
import { toast } from './toast.svelte';

type LibraryConfig = {
  roots?: string[];
  recursive?: boolean;
  thumbnail_size?: number;
  auto_scan?: boolean;
};
type ConfigShape = { library?: LibraryConfig } & Record<string, unknown>;

export type AspectFilter = 'all' | 'wide' | 'square' | 'portrait';
export type SortKey = 'date_added' | 'date_modified' | 'resolution' | 'last_shown' | 'name';

// Per-request page size. A LibraryEntry serialises to ~200–300 bytes of JSON,
// so 1 500 entries lands at ~300–450 KiB — comfortably under the 1 MiB frame
// cap even when paths are long. Lower this if `library_list` ever starts
// returning richer per-entry payloads.
const PAGE_SIZE = 1500;
// Hard upper bound on entries the UI will hold in memory at once. The
// virtualised grid mounts only the visible row range, but the in-memory
// `entries` array still gets walked on every filter change. 60 000 is high
// enough for any reasonable on-disk wallpaper collection without making
// `visible()` noticeably slow.
const MAX_ENTRIES = 60_000;

let entries = $state<LibraryEntry[]>([]);
let roots = $state<string[]>([]);
let loading = $state(false);
let busyRoots = $state(false);
let search = $state('');
let activeTag = $state<string | null>(null);
let activeRoot = $state<string | null>(null);
let aspect = $state<AspectFilter>('all');
let minResolution = $state(0);
let sort = $state<SortKey>('date_added');
let favouritesOnly = $state(false);

function unixSecs(field: LibraryEntry['modified'] | LibraryEntry['last_shown']): number {
  if (field === null || field === undefined) return 0;
  if (typeof field === 'number') return field;
  if (typeof field === 'string') {
    const n = Date.parse(field);
    return Number.isFinite(n) ? n / 1000 : 0;
  }
  if (typeof field === 'object' && 'secs_since_epoch' in field) {
    return field.secs_since_epoch;
  }
  return 0;
}

function fileName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i >= 0 ? path.slice(i + 1) : path;
}

function matchesSearch(entry: LibraryEntry, q: string): boolean {
  if (!q) return true;
  const needle = q.toLowerCase();
  if (fileName(entry.path).toLowerCase().includes(needle)) return true;
  return entry.tags.some((t) => t.toLowerCase().includes(needle));
}

// Roots are stored in their config form (often `~/…`) while entry paths are
// absolute, so match on the tilde-stripped tail at a directory boundary. A
// nested folder sharing a root's leaf name elsewhere could in theory match, but
// that's an acceptable edge for a folder filter.
function inRoot(path: string, root: string): boolean {
  const tail = root.replace(/^~/, '').replace(/\/+$/, '');
  if (!tail) return true;
  return (path + '/').includes(tail + '/');
}

function matchesAspect(ratio: number, mode: AspectFilter): boolean {
  if (mode === 'all') return true;
  if (mode === 'wide') return ratio > 1.4;
  if (mode === 'portrait') return ratio < 0.75;
  // square: 0.75..1.4
  return ratio >= 0.75 && ratio <= 1.4;
}

function compare(a: LibraryEntry, b: LibraryEntry, key: SortKey): number {
  switch (key) {
    case 'date_modified':
      return unixSecs(b.modified) - unixSecs(a.modified);
    case 'resolution':
      return b.resolution[0] * b.resolution[1] - a.resolution[0] * a.resolution[1];
    case 'last_shown':
      return unixSecs(b.last_shown) - unixSecs(a.last_shown);
    case 'name':
      return fileName(a.path).localeCompare(fileName(b.path));
    case 'date_added':
    default:
      // No "added at" column yet; fall back to mtime which is the next-best
      // proxy for "showed up in the library recently".
      return unixSecs(b.modified) - unixSecs(a.modified);
  }
}

function visible(): LibraryEntry[] {
  const out = entries.filter((e) => {
    if (favouritesOnly && !e.favourite) return false;
    if (activeTag && !e.tags.some((t) => t.toLowerCase() === activeTag?.toLowerCase()))
      return false;
    if (activeRoot && roots.includes(activeRoot) && !inRoot(e.path, activeRoot)) return false;
    if (!matchesAspect(e.aspect_ratio, aspect)) return false;
    if (minResolution > 0 && Math.min(e.resolution[0], e.resolution[1]) < minResolution)
      return false;
    if (!matchesSearch(e, search)) return false;
    return true;
  });
  out.sort((a, b) => compare(a, b, sort));
  return out;
}

function allTags(): string[] {
  const set = new Set<string>();
  for (const e of entries) for (const t of e.tags) set.add(t.toLowerCase());
  return [...set].sort();
}

// Number of thumbnails to prewarm into `thumb-cache` after a refresh. Sized to
// cover the initial viewport of `LibraryModal` (~4 cols × ~4 rows at the
// default 1100×720 modal size) plus headroom for taller windows. Without this,
// the first time the modal opens the visible thumbs each fire their own
// `library_thumbnail` IPC roundtrip on mount, which adds a ~200 ms paint
// stutter even when the entry list itself is already in memory.
const PREWARM_THUMBS = 32;

// Concurrency bound for the prewarm fan-out. The thumbnail IPC opens a file,
// decodes it, and rescales — saturating the daemon's worker pool with 32
// parallel requests caused noticeable apply-time stalls on slower disks.
// Six in flight at a time keeps the daemon responsive and still finishes
// the prewarm well before the user can scroll.
const PREWARM_CONCURRENCY = 6;

async function prewarmThumbnails(list: LibraryEntry[]): Promise<void> {
  const head = [...list].sort((a, b) => compare(a, b, 'date_added')).slice(0, PREWARM_THUMBS);
  for (let i = 0; i < head.length; i += PREWARM_CONCURRENCY) {
    const chunk = head.slice(i, i + PREWARM_CONCURRENCY);
    await Promise.allSettled(chunk.map((e) => getLibraryThumbUrl(e.path)));
  }
}

// Walk `library_list` pages until either the daemon returns a short page (no
// more entries) or we hit MAX_ENTRIES. Avoids the 1 MiB IPC frame cap that a
// single huge-limit request would smash into for libraries of more than a few
// thousand long-path images.
async function fetchAllEntries(): Promise<LibraryEntry[]> {
  const out: LibraryEntry[] = [];
  while (out.length < MAX_ENTRIES) {
    const page = await api.libraryList({
      limit: PAGE_SIZE,
      offset: out.length,
    } satisfies LibraryFilter);
    out.push(...page);
    if (page.length < PAGE_SIZE) return out;
  }
  toast.error(
    'Library truncated',
    `Showing the first ${MAX_ENTRIES.toLocaleString()} entries; narrow your library roots to see the rest.`,
  );
  return out;
}

export const libraryStore = {
  get entries() {
    return entries;
  },
  get visible() {
    return visible();
  },
  get loading() {
    return loading;
  },
  get search() {
    return search;
  },
  set search(v: string) {
    search = v;
  },
  get activeTag() {
    return activeTag;
  },
  set activeTag(v: string | null) {
    activeTag = v;
  },
  get activeRoot() {
    return activeRoot;
  },
  set activeRoot(v: string | null) {
    activeRoot = v;
  },
  get aspect() {
    return aspect;
  },
  set aspect(v: AspectFilter) {
    aspect = v;
  },
  get minResolution() {
    return minResolution;
  },
  set minResolution(v: number) {
    minResolution = Math.max(0, v | 0);
  },
  get sort() {
    return sort;
  },
  set sort(v: SortKey) {
    sort = v;
  },
  get favouritesOnly() {
    return favouritesOnly;
  },
  set favouritesOnly(v: boolean) {
    favouritesOnly = v;
  },
  get tags() {
    return allTags();
  },
  get roots() {
    return roots;
  },
  get busyRoots() {
    return busyRoots;
  },

  async refresh() {
    loading = true;
    try {
      const [list, config] = await Promise.all([
        fetchAllEntries(),
        api.getConfig() as Promise<ConfigShape>,
      ]);
      entries = list;
      roots = config.library?.roots ?? [];
      void prewarmThumbnails(list);
    } catch (err) {
      toast.error('Could not load library', errorMessage(err));
      entries = [];
    } finally {
      loading = false;
    }
  },

  async rescan() {
    loading = true;
    try {
      const r = await api.libraryRescan();
      entries = await fetchAllEntries();
      prewarmThumbnails(entries);
      toast.success('Library rescanned', `${r.count} entries`);
    } catch (err) {
      toast.error('Rescan failed', errorMessage(err));
    } finally {
      loading = false;
    }
  },

  async addRoot(path: string) {
    if (!path) return;
    busyRoots = true;
    try {
      const config = (await api.getConfig()) as ConfigShape;
      const existing = config.library?.roots ?? [];
      if (existing.includes(path)) {
        toast.error('Already added', `${path} is already a library root`);
        return;
      }
      const next: ConfigShape = {
        ...config,
        library: { ...(config.library ?? {}), roots: [...existing, path] },
      };
      await api.saveConfig(next);
      roots = next.library?.roots ?? [];
      toast.success('Folder added', path);
      await this.rescan();
    } catch (err) {
      toast.error('Could not add folder', errorMessage(err));
    } finally {
      busyRoots = false;
    }
  },

  async removeRoot(path: string) {
    busyRoots = true;
    try {
      const config = (await api.getConfig()) as ConfigShape;
      const existing = config.library?.roots ?? [];
      const next: ConfigShape = {
        ...config,
        library: { ...(config.library ?? {}), roots: existing.filter((r) => r !== path) },
      };
      await api.saveConfig(next);
      roots = next.library?.roots ?? [];
      toast.success('Folder removed', path);
      await this.rescan();
    } catch (err) {
      toast.error('Could not remove folder', errorMessage(err));
    } finally {
      busyRoots = false;
    }
  },

  async setTag(path: string, tag: string, on: boolean) {
    try {
      await api.libraryTag(path, tag, on);
      const e = entries.find((x) => x.path === path);
      if (!e) return;
      if (tag.toLowerCase() === 'favourite') {
        e.favourite = on;
      } else {
        const norm = tag.trim().toLowerCase();
        const idx = e.tags.findIndex((t) => t.toLowerCase() === norm);
        if (on && idx < 0) e.tags = [...e.tags, norm];
        if (!on && idx >= 0) e.tags = e.tags.filter((_, i) => i !== idx);
      }
    } catch (err) {
      toast.error('Tag update failed', errorMessage(err));
    }
  },

  async toggleFavourite(path: string) {
    const e = entries.find((x) => x.path === path);
    if (!e) return;
    await this.setTag(path, 'favourite', !e.favourite);
  },

  async remove(path: string) {
    try {
      await api.libraryDelete(path);
      entries = entries.filter((e) => e.path !== path);
    } catch (err) {
      toast.error('Could not remove from library', errorMessage(err));
    }
  },
};
