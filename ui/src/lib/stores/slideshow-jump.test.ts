import { describe, expect, it } from 'vitest';

import type { LibraryEntry } from '$lib/api';
import type { SlideshowSource } from '$lib/types/profile-helpers';
import { libraryIntersection } from './slideshow-jump.svelte';

function entry(path: string): LibraryEntry {
  return {
    path,
    resolution: [1, 1],
    aspect_ratio: 1,
    file_size: 0,
    modified: 0,
    tags: [],
    favourite: false,
    last_shown: null,
    show_count: 0,
  };
}

const source: SlideshowSource = {
  images: { sources: [{ type: 'folder', path: '/walls', recursive: false }] },
  config: {
    interval_secs: 60,
    sort: 'alphabetical',
    recent_history_size: 4,
    on_start: 'resume',
    pause_when_active: false,
    skip_on_unavailable: false,
  },
};

describe('libraryIntersection', () => {
  it('keeps only entries covered by the slideshow set, sorted by path', () => {
    const entries = [entry('/walls/b.png'), entry('/elsewhere/c.png'), entry('/walls/a.png')];
    expect(libraryIntersection(source, entries)).toEqual(['/walls/a.png', '/walls/b.png']);
  });

  it('returns an empty list when nothing in the library is covered', () => {
    const entries = [entry('/elsewhere/c.png')];
    expect(libraryIntersection(source, entries)).toEqual([]);
  });

  it('excludes non-recursive folder subdirectories, matching daemon semantics', () => {
    const entries = [entry('/walls/sub/deep.png'), entry('/walls/top.png')];
    expect(libraryIntersection(source, entries)).toEqual(['/walls/top.png']);
  });
});
