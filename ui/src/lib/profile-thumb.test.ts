import { describe, expect, it } from 'vitest';
import type { Profile } from '$lib/api';
import type { ProfileBody } from '$lib/types/profile-helpers';
import { profileThumbPath } from './profile-thumb';

function profile(body: ProfileBody): Profile {
  return {
    name: 'p',
    body,
    monitor_state: {},
    topology: 'topo-1',
    description: null,
    created_at: '2026-06-11T00:00:00Z',
    updated_at: '2026-06-11T00:00:00Z',
    last_applied_at: null,
    backend_override: null,
  };
}

const rect = { x_mm: 0, y_mm: 0, w_mm: 1000, h_mm: 600 };

function slideshow(sources: { type: 'image' | 'folder'; path: string }[]): Profile {
  return profile({
    type: 'slideshow',
    source: {
      images: {
        sources: sources.map((s) =>
          s.type === 'folder'
            ? { ...s, type: 'folder', recursive: false }
            : { ...s, type: 'image' },
        ),
      },
      config: {
        interval_secs: 600,
        sort: 'shuffle',
        recent_history_size: 10,
        on_start: 'resume',
        pause_when_active: false,
        skip_on_unavailable: true,
      },
    },
    image_rect_mm: rect,
  });
}

describe('profileThumbPath', () => {
  it('uses a standard profile top layer path directly', () => {
    const p = profile({
      type: 'standard',
      layers: [{ path: '/walls/a.png', image_rect_mm: rect }],
    });
    expect(profileThumbPath(p, [])).toBe('/walls/a.png');
  });

  it('prefers the first hand-picked slideshow image', () => {
    const p = slideshow([
      { type: 'folder', path: '/walls' },
      { type: 'image', path: '/pick/z.png' },
    ]);
    expect(profileThumbPath(p, ['/walls/a.png'])).toBe('/pick/z.png');
  });

  it('falls back to the alphabetically first library entry under a folder source', () => {
    const p = slideshow([{ type: 'folder', path: '/walls' }]);
    const library = ['/elsewhere/x.png', '/walls/b.png', '/walls/a.png'];
    expect(profileThumbPath(p, library)).toBe('/walls/a.png');
  });

  it('returns null when nothing in the library covers the set', () => {
    const p = slideshow([{ type: 'folder', path: '/walls' }]);
    expect(profileThumbPath(p, ['/elsewhere/x.png'])).toBeNull();
  });

  it('rejects relative paths', () => {
    const p = profile({
      type: 'standard',
      layers: [{ path: 'walls/a.png', image_rect_mm: rect }],
    });
    expect(profileThumbPath(p, [])).toBeNull();
  });
});
