import { describe, expect, it } from 'vitest';
import { createDragRegionPublisher, type DragRect } from './window-drag';

const rect = (x: number): DragRect => ({ x, y: 0, w: 100, h: 40 });

describe('createDragRegionPublisher', () => {
  it('first_regions_are_sent', () => {
    const sent: DragRect[][] = [];
    const publish = createDragRegionPublisher(async (r) => void sent.push(r));
    publish([rect(0)]);
    expect(sent).toEqual([[rect(0)]]);
  });

  it('unchanged_regions_are_not_resent', () => {
    const sent: DragRect[][] = [];
    const publish = createDragRegionPublisher(async (r) => void sent.push(r));
    publish([rect(0)]);
    publish([rect(0)]);
    publish([rect(0)]);
    expect(sent).toHaveLength(1);
  });

  it('moved_region_is_resent', () => {
    const sent: DragRect[][] = [];
    const publish = createDragRegionPublisher(async (r) => void sent.push(r));
    publish([rect(0)]);
    publish([rect(12)]);
    expect(sent).toEqual([[rect(0)], [rect(12)]]);
  });

  it('overlay_clears_regions_and_reopening_restores_them', () => {
    const sent: DragRect[][] = [];
    const publish = createDragRegionPublisher(async (r) => void sent.push(r));
    publish([rect(0)]);
    publish([]);
    publish([rect(0)]);
    expect(sent).toEqual([[rect(0)], [], [rect(0)]]);
  });

  it('failed_send_is_retried_on_the_next_publish', async () => {
    const sent: DragRect[][] = [];
    const publish = createDragRegionPublisher(async (r) => {
      sent.push(r);
      throw new Error('ipc down');
    });
    publish([rect(0)]);
    await Promise.resolve();
    publish([rect(0)]);
    expect(sent).toHaveLength(2);
  });
});
