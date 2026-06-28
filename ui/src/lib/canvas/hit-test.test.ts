import { describe, it, expect } from 'vitest';
import { hitTest, cursorFor, type HitGeometry } from './hit-test';

const rect = (x: number, y: number, w: number, h: number) => ({ x, y, w, h });

function geo(over: Partial<HitGeometry>): HitGeometry {
  return {
    compositeMode: false,
    imagesInteractive: true,
    buttonsForLayer: () => true,
    layerRects: [],
    monitors: [],
    imageUrl: null,
    imgRect: rect(0, 0, 0, 0),
    ...over,
  };
}

describe('hitTest', () => {
  it('composite_layers_sit_above_monitors', () => {
    const g = geo({
      compositeMode: true,
      monitors: [{ id: 'm', rect: rect(0, 0, 200, 200) }],
      layerRects: [{ id: 'L', rect: rect(0, 0, 200, 200) }],
    });
    expect(hitTest(100, 100, g)).toEqual({ type: 'layer', id: 'L' });
  });

  it('topmost_layer_wins_when_two_overlap', () => {
    const g = geo({
      compositeMode: true,
      layerRects: [
        { id: 'bottom', rect: rect(0, 0, 200, 200) },
        { id: 'top', rect: rect(0, 0, 200, 200) },
      ],
    });
    expect(hitTest(100, 100, g)).toEqual({ type: 'layer', id: 'top' });
  });

  it('layer_remove_region_takes_priority_over_body', () => {
    const g = geo({ compositeMode: true, layerRects: [{ id: 'L', rect: rect(0, 0, 200, 200) }] });
    // ✕ centre is (x+w-20, y+20) = (180, 20).
    expect(hitTest(180, 20, g)).toEqual({ type: 'layer-remove', id: 'L' });
  });

  it('layer_snap_regions_sit_left_of_remove', () => {
    const g = geo({ compositeMode: true, layerRects: [{ id: 'L', rect: rect(0, 0, 200, 200) }] });
    // snap-width centre is (x+w-46, y+20) = (154, 20); height is (128, 20).
    expect(hitTest(154, 20, g)).toEqual({ type: 'layer-snap', id: 'L', axis: 'width' });
    expect(hitTest(128, 20, g)).toEqual({ type: 'layer-snap', id: 'L', axis: 'height' });
  });

  it('monitor_mode_makes_layers_click_through', () => {
    const g = geo({
      compositeMode: true,
      imagesInteractive: false,
      monitors: [{ id: 'm', rect: rect(0, 0, 200, 200) }],
      layerRects: [{ id: 'L', rect: rect(0, 0, 200, 200) }],
    });
    expect(hitTest(100, 100, g)).toEqual({ type: 'monitor', id: 'm' });
  });

  it('small_layers_skip_their_buttons', () => {
    const g = geo({
      compositeMode: true,
      buttonsForLayer: () => false,
      layerRects: [{ id: 'L', rect: rect(0, 0, 200, 200) }],
    });
    // The remove centre now falls through to the layer body.
    expect(hitTest(188, 20, g)).toEqual({ type: 'layer', id: 'L' });
  });

  it('falls_through_to_monitor_in_uncovered_area', () => {
    const g = geo({
      compositeMode: true,
      monitors: [{ id: 'm', rect: rect(0, 0, 400, 400) }],
      layerRects: [{ id: 'L', rect: rect(0, 0, 100, 100) }],
    });
    expect(hitTest(300, 300, g)).toEqual({ type: 'monitor', id: 'm' });
  });

  it('span_image_only_hit_when_not_composite', () => {
    const g = geo({ imageUrl: 'data:x', imgRect: rect(0, 0, 200, 200) });
    expect(hitTest(100, 100, g)).toEqual({ type: 'image' });
  });

  it('empty_stage_returns_stage', () => {
    expect(hitTest(50, 50, geo({}))).toEqual({ type: 'stage' });
  });
});

describe('cursorFor', () => {
  it('maps_hit_types_to_cursors', () => {
    expect(cursorFor({ type: 'layer', id: 'a' })).toBe('move');
    expect(cursorFor({ type: 'layer-remove', id: 'a' })).toBe('pointer');
    expect(cursorFor({ type: 'stage' })).toBe('default');
  });
});
