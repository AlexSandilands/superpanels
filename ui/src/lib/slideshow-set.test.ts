import { describe, expect, it } from 'vitest';

import {
  addFolder,
  addImage,
  emptyImageSet,
  membership,
  removeImage,
  removeSourceAt,
  sourceLabel,
} from './slideshow-set';
import type { ImageSet } from '$lib/types/ImageSet';

const mixed: ImageSet = {
  sources: [
    { type: 'folder', path: '/walls', recursive: false },
    { type: 'folder', path: '/deep', recursive: true },
    { type: 'image', path: '/pick/a.png' },
  ],
};

describe('membership', () => {
  it('reports direct images as image even when also folder-covered', () => {
    const set = addImage(mixed, '/walls/x.png');
    expect(membership(set, '/walls/x.png')).toBe('image');
  });

  it('reports folder coverage for direct children of a non-recursive folder', () => {
    expect(membership(mixed, '/walls/x.png')).toBe('folder');
    expect(membership(mixed, '/walls/sub/x.png')).toBeNull();
  });

  it('reports folder coverage at any depth for recursive folders', () => {
    expect(membership(mixed, '/deep/a/b/c.png')).toBe('folder');
  });

  it('does not match sibling folders sharing a name prefix', () => {
    expect(membership(mixed, '/walls-extra/x.png')).toBeNull();
  });

  it('returns null for paths outside every source', () => {
    expect(membership(emptyImageSet(), '/anything.png')).toBeNull();
  });

  it('tolerates trailing slashes and doubled separators like the daemon does', () => {
    const set: ImageSet = {
      sources: [{ type: 'folder', path: '/walls/', recursive: false }],
    };
    expect(membership(set, '/walls/x.png')).toBe('folder');
    expect(membership(set, '/walls//x.png')).toBe('folder');
  });
});

describe('add / remove', () => {
  it('addImage is idempotent', () => {
    const once = addImage(emptyImageSet(), '/a.png');
    const twice = addImage(once, '/a.png');
    expect(twice.sources).toHaveLength(1);
  });

  it('removeImage removes only the matching image source', () => {
    const set = removeImage(mixed, '/pick/a.png');
    expect(set.sources).toHaveLength(2);
    expect(set.sources.every((s) => s.type === 'folder')).toBe(true);
  });

  it('addFolder is idempotent on the same path', () => {
    const set = addFolder(mixed, '/walls', true);
    expect(set.sources).toHaveLength(3);
  });

  it('removeSourceAt removes by position', () => {
    const set = removeSourceAt(mixed, 0);
    expect(set.sources[0]).toEqual({ type: 'folder', path: '/deep', recursive: true });
  });

  it('mutating helpers leave the input set untouched', () => {
    addImage(mixed, '/new.png');
    removeImage(mixed, '/pick/a.png');
    expect(mixed.sources).toHaveLength(3);
  });
});

describe('sourceLabel', () => {
  it('labels folders with a trailing slash and images by file name', () => {
    expect(sourceLabel({ type: 'folder', path: '/home/me/walls', recursive: true })).toBe('walls/');
    expect(sourceLabel({ type: 'image', path: '/pick/a.png' })).toBe('a.png');
  });
});
