import { describe, it, expect } from 'vitest';
import { hNeighbourPairs, normaliseAxis, uniformGap, vNeighbourPairs } from './gaps';
import type { PreviewMonitor } from './projection';

function pm(over: Partial<PreviewMonitor> & { id: string }): PreviewMonitor {
  const base: PreviewMonitor = {
    id: over.id,
    name: over.id,
    model: '',
    refreshHz: 60,
    rotation: 0,
    nativeWmm: over.wMm ?? 500,
    nativeHmm: over.hMm ?? 300,
    nativePxW: 1920,
    nativePxH: 1080,
    wMm: over.wMm ?? 500,
    hMm: over.hMm ?? 300,
    pxW: 1920,
    pxH: 1080,
    xMm: over.xMm ?? 0,
    yMm: over.yMm ?? 0,
    missing: false,
  };
  return { ...base, ...over };
}

describe('hNeighbourPairs', () => {
  it('reports_a_single_pair_for_two_side_by_side_monitors', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 510, yMm: 0, wMm: 500, hMm: 300 });
    const pairs = hNeighbourPairs([a, b]);
    expect(pairs).toHaveLength(1);
    expect(pairs[0]?.gapMm).toBe(10);
  });

  it('skips_pair_when_a_third_monitor_lies_between_them', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const middle = pm({ id: 'M', xMm: 510, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 1020, yMm: 0, wMm: 500, hMm: 300 });
    const pairs = hNeighbourPairs([a, middle, b]);
    // A→M and M→B are direct neighbours; A→B is excluded by the "between" check.
    expect(pairs.map((p) => `${p.a.id}->${p.b.id}`).sort()).toEqual(['A->M', 'M->B']);
  });

  it('requires_y_overlap_above_the_epsilon', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const offset = pm({ id: 'B', xMm: 510, yMm: 301, wMm: 500, hMm: 300 });
    expect(hNeighbourPairs([a, offset])).toHaveLength(0);
  });

  it('treats_overlap_within_epsilon_as_aligned', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    // 0.4 mm shift — under the 0.5 mm GAP_EPS — should still pair.
    const b = pm({ id: 'B', xMm: 510, yMm: 0.4, wMm: 500, hMm: 300 });
    expect(hNeighbourPairs([a, b])).toHaveLength(1);
  });
});

describe('vNeighbourPairs', () => {
  it('reports_pair_when_b_is_below_a_with_x_overlap', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 0, yMm: 310, wMm: 500, hMm: 300 });
    const pairs = vNeighbourPairs([a, b]);
    expect(pairs).toHaveLength(1);
    expect(pairs[0]?.gapMm).toBe(10);
  });

  it('skips_when_third_monitor_lies_between_vertically', () => {
    const top = pm({ id: 'T', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const middle = pm({ id: 'M', xMm: 0, yMm: 310, wMm: 500, hMm: 300 });
    const bottom = pm({ id: 'B', xMm: 0, yMm: 620, wMm: 500, hMm: 300 });
    const pairs = vNeighbourPairs([top, middle, bottom]);
    expect(pairs.map((p) => `${p.a.id}->${p.b.id}`).sort()).toEqual(['M->B', 'T->M']);
  });
});

describe('uniformGap', () => {
  it('returns_null_for_no_pairs', () => {
    expect(uniformGap([])).toBeNull();
  });

  it('returns_value_when_all_pairs_agree_within_tolerance', () => {
    const a = pm({ id: 'A' });
    const b = pm({ id: 'B' });
    const pairs = [
      { a, b, gapMm: 10 },
      { a, b, gapMm: 10.3 },
    ];
    expect(uniformGap(pairs, 0.5)).toBe(10);
  });

  it('returns_null_when_pairs_disagree_outside_tolerance', () => {
    const a = pm({ id: 'A' });
    const b = pm({ id: 'B' });
    const pairs = [
      { a, b, gapMm: 10 },
      { a, b, gapMm: 12 },
    ];
    expect(uniformGap(pairs, 0.5)).toBeNull();
  });
});

describe('normaliseAxis', () => {
  it('respaces_chained_h_neighbours_to_target_gap', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 0, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 600, yMm: 0, wMm: 500, hMm: 300 });
    const c = pm({ id: 'C', xMm: 1300, yMm: 0, wMm: 500, hMm: 300 });
    const out = normaliseAxis([a, b, c], {}, 10, hNeighbourPairs, 'x');
    expect(out['A']?.xMm).toBe(0); // head untouched
    expect(out['B']?.xMm).toBe(510); // 0 + 500 + 10
    expect(out['C']?.xMm).toBe(1020); // 510 + 500 + 10
  });

  it('preserves_existing_overrides_for_axes_it_does_not_touch', () => {
    const a = pm({ id: 'A', xMm: 0, yMm: 100, wMm: 500, hMm: 300 });
    const b = pm({ id: 'B', xMm: 600, yMm: 100, wMm: 500, hMm: 300 });
    const before = { B: { xMm: 600, yMm: 100 } };
    const out = normaliseAxis([a, b], before, 0, hNeighbourPairs, 'x');
    expect(out['B']?.yMm).toBe(100);
    expect(out['B']?.xMm).toBe(500);
  });
});
