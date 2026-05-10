import { describe, it, expect } from 'vitest';
import type { Monitor } from '$lib/api';
import {
  buildPreviewMonitors,
  coverImageRect,
  defaultOverrides,
  rotationDeg,
  stableId,
} from './projection';

function monitor(over: Partial<Monitor> = {}): Monitor {
  return {
    id: 0,
    name: 'DP-1',
    stable_id: 'kde-uuid-DP-1',
    position: [0, 0],
    resolution: [1920, 1080],
    physical_size_mm: [527, 296],
    scale: 1,
    rotation: 'none',
    refresh_hz: 60,
    primary: false,
    ppi: 92,
    ...over,
  };
}

describe('stableId', () => {
  it('uses stable_id_when_present', () => {
    expect(stableId(monitor({ stable_id: 'kde-uuid', name: 'DP-1' }))).toBe('kde-uuid');
  });

  it('falls_back_to_name_when_stable_id_empty', () => {
    expect(stableId(monitor({ stable_id: '', name: 'DP-1' }))).toBe('DP-1');
  });

  it('falls_back_to_name_when_stable_id_null', () => {
    expect(stableId(monitor({ stable_id: null, name: 'HDMI-A-1' }))).toBe('HDMI-A-1');
  });
});

describe('rotationDeg', () => {
  it('maps_each_rotation_variant_to_degrees', () => {
    expect(rotationDeg(monitor({ rotation: 'none' }))).toBe(0);
    expect(rotationDeg(monitor({ rotation: 'right' }))).toBe(90);
    expect(rotationDeg(monitor({ rotation: 'inverted' }))).toBe(180);
    expect(rotationDeg(monitor({ rotation: 'left' }))).toBe(270);
  });
});

describe('defaultOverrides', () => {
  it('lays_out_in_compositor_order_with_bezel_gaps', () => {
    const m1 = monitor({
      stable_id: 'a',
      name: 'A',
      position: [0, 0],
      physical_size_mm: [500, 300],
    });
    const m2 = monitor({
      stable_id: 'b',
      name: 'B',
      position: [1920, 0],
      physical_size_mm: [500, 300],
    });
    const out = defaultOverrides([m1, m2], 10);
    expect(out['a']).toEqual({ xMm: 0, yMm: 0 });
    expect(out['b']).toEqual({ xMm: 510, yMm: 0 });
  });

  it('uses_dpi_fallback_when_physical_size_missing', () => {
    const m = monitor({
      stable_id: 'a',
      physical_size_mm: null,
      resolution: [1920, 1080],
    });
    const out = defaultOverrides([m], 0);
    // 1920 px / 96 dpi * 25.4 = 508 mm — next monitor (none) lands at 508+0
    expect(out['a']?.xMm).toBe(0);
  });

  it('rotated_monitors_use_swapped_dimensions_for_cursor_advance', () => {
    const portrait = monitor({
      stable_id: 'p',
      rotation: 'right',
      physical_size_mm: [500, 300],
      position: [0, 0],
    });
    const next = monitor({
      stable_id: 'n',
      physical_size_mm: [500, 300],
      position: [1080, 0],
    });
    const out = defaultOverrides([portrait, next], 0);
    // Rotated: width becomes 300 (was-h), so 'n' lands at xMm=300.
    expect(out['n']?.xMm).toBe(300);
  });

  it('sorts_by_position_y_then_x', () => {
    const top = monitor({ stable_id: 't', position: [100, 0], physical_size_mm: [500, 300] });
    const bottom = monitor({ stable_id: 'b', position: [0, 100], physical_size_mm: [500, 300] });
    const out = defaultOverrides([bottom, top], 0);
    // Top sorts first → xMm=0; bottom is laid after at xMm=500.
    expect(out['t']?.xMm).toBe(0);
    expect(out['b']?.xMm).toBe(500);
  });
});

describe('buildPreviewMonitors', () => {
  it('mixes_landscape_and_portrait_with_correct_effective_dims', () => {
    const land = monitor({
      stable_id: 'L',
      resolution: [1920, 1080],
      physical_size_mm: [500, 300],
      rotation: 'none',
    });
    const port = monitor({
      stable_id: 'P',
      resolution: [1920, 1080],
      physical_size_mm: [500, 300],
      rotation: 'right',
    });
    const out = buildPreviewMonitors([land, port], {
      L: { xMm: 0, yMm: 0 },
      P: { xMm: 600, yMm: 0 },
    });
    expect(out[0]?.wMm).toBe(500);
    expect(out[0]?.hMm).toBe(300);
    expect(out[1]?.wMm).toBe(300); // portrait → swapped
    expect(out[1]?.hMm).toBe(500);
    expect(out[1]?.pxW).toBe(1080);
    expect(out[1]?.pxH).toBe(1920);
  });

  it('flags_missing_physical_size_with_dpi_fallback', () => {
    const m = monitor({ stable_id: 'a', physical_size_mm: null, resolution: [1920, 1080] });
    const out = buildPreviewMonitors([m], { a: { xMm: 0, yMm: 0 } });
    expect(out[0]?.missing).toBe(true);
  });
});

describe('coverImageRect', () => {
  it('width_dominates_when_image_is_wider_than_bbox', () => {
    const monitors = [{ id: 'a', xMm: 0, yMm: 0, wMm: 1000, hMm: 500 } as never];
    const rect = coverImageRect(monitors, 4 / 1); // very wide
    // bbox is 1000×500 (aspect 2). Image aspect 4 → use bbW (1000), height = 250 < 500, so height-dominates.
    expect(rect.heightMm).toBe(500);
    expect(rect.widthMm).toBe(2000); // 4 * 500
    // Centred horizontally: offset = 0 + 500 - 1000 = -500.
    expect(rect.offsetMmX).toBe(-500);
    expect(rect.offsetMmY).toBe(0);
  });

  it('height_dominates_when_image_is_narrower_than_bbox', () => {
    const monitors = [{ id: 'a', xMm: 0, yMm: 0, wMm: 500, hMm: 500 } as never];
    const rect = coverImageRect(monitors, 1 / 2); // tall
    // bbW=500, h = 500/0.5 = 1000 ≥ bbH(500), so width-dominates.
    expect(rect.widthMm).toBe(500);
    expect(rect.heightMm).toBe(1000);
  });

  it('empty_monitor_list_returns_arbitrary_default_aspect_match', () => {
    const rect = coverImageRect([], 16 / 9);
    expect(rect.widthMm).toBeGreaterThan(0);
    expect(rect.heightMm).toBeGreaterThan(0);
    expect(rect.widthMm / rect.heightMm).toBeCloseTo(16 / 9, 5);
  });
});
