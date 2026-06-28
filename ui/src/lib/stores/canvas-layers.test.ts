import { describe, it, expect, beforeEach, vi } from 'vitest';

vi.mock('$lib/library/source-image', () => ({
  peekSourceImage: () => null,
  loadSourceImage: (path: string) =>
    Promise.resolve({ url: `data:url/${path}`, naturalW: 1920, naturalH: 1080 }),
}));
vi.mock('$lib/stores/toast.svelte', () => ({ toast: { error: () => {} } }));

import { canvasLayers } from './canvas-layers.svelte';
import type { StandardLayer } from '$lib/types/profile-helpers';

function layer(path: string, x: number): StandardLayer {
  return { path, image_rect_mm: { x_mm: x, y_mm: 0, w_mm: 100, h_mm: 100 } };
}

beforeEach(() => {
  canvasLayers.clear();
});

describe('canvasLayers', () => {
  it('setFromLayers_then_toLayers_round_trips_paths_and_rects', () => {
    canvasLayers.setFromLayers([layer('/a.png', 0), layer('/b.png', 200)]);
    const out = canvasLayers.toLayers();
    expect(out.map((l) => l.path)).toEqual(['/a.png', '/b.png']);
    expect(out[1]?.image_rect_mm.x_mm).toBe(200);
  });

  it('add_appends_on_top_and_loads_the_image', async () => {
    await canvasLayers.add('/big.png', []);
    await canvasLayers.add('/small.png', []);
    const list = canvasLayers.list;
    expect(list.map((l) => l.path)).toEqual(['/big.png', '/small.png']);
    // Top (last) layer is the most recently added.
    expect(list[1]?.url).toBe('data:url//small.png');
  });

  it('remove_drops_only_the_matching_layer', () => {
    canvasLayers.setFromLayers([layer('/a.png', 0), layer('/b.png', 0)]);
    const id = canvasLayers.list[0]?.id ?? '';
    canvasLayers.remove(id);
    expect(canvasLayers.list.map((l) => l.path)).toEqual(['/b.png']);
  });

  it('bringToFront_moves_a_layer_to_the_end', () => {
    canvasLayers.setFromLayers([layer('/a.png', 0), layer('/b.png', 0), layer('/c.png', 0)]);
    const aId = canvasLayers.list[0]?.id ?? '';
    canvasLayers.bringToFront(aId);
    expect(canvasLayers.list.map((l) => l.path)).toEqual(['/b.png', '/c.png', '/a.png']);
  });

  it('patch_updates_one_layers_transform', () => {
    canvasLayers.setFromLayers([layer('/a.png', 0)]);
    const id = canvasLayers.list[0]?.id ?? '';
    canvasLayers.patch(id, { offsetMmX: 50, offsetMmY: 60, widthMm: 300, heightMm: 200 });
    expect(canvasLayers.toLayers()[0]?.image_rect_mm).toEqual({
      x_mm: 50,
      y_mm: 60,
      w_mm: 300,
      h_mm: 200,
    });
  });
});
