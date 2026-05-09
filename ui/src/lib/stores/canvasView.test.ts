import { describe, it, expect, beforeEach } from 'vitest';
import { canvasView, type MonitorOverride } from './canvasView.svelte';

function ovr(xMm: number, yMm: number, rotation: 0 | 90 | 180 | 270 = 0): MonitorOverride {
  return { xMm, yMm, rotation };
}

beforeEach(() => {
  canvasView.setOverrides({});
});

describe('canvasView.hasOverrides', () => {
  it('returns_false_when_overrides_match_defaults_exactly', () => {
    const defaults = { A: ovr(0, 0), B: ovr(500, 0) };
    canvasView.setOverrides({ A: ovr(0, 0), B: ovr(500, 0) });
    expect(canvasView.hasOverrides(defaults)).toBe(false);
  });

  it('returns_false_for_displacements_within_half_mm', () => {
    const defaults = { A: ovr(0, 0) };
    canvasView.setOverrides({ A: ovr(0.4, 0.4) });
    expect(canvasView.hasOverrides(defaults)).toBe(false);
  });

  it('returns_true_just_above_the_epsilon', () => {
    const defaults = { A: ovr(0, 0) };
    canvasView.setOverrides({ A: ovr(0.6, 0) });
    expect(canvasView.hasOverrides(defaults)).toBe(true);
  });

  it('returns_true_when_rotation_changes_even_with_zero_displacement', () => {
    const defaults = { A: ovr(0, 0, 0) };
    canvasView.setOverrides({ A: ovr(0, 0, 90) });
    expect(canvasView.hasOverrides(defaults)).toBe(true);
  });

  it('skips_ids_missing_from_either_side', () => {
    const defaults = { A: ovr(0, 0), B: ovr(500, 0) };
    // B has no override entry; A matches. Should not falsely trigger via B.
    canvasView.setOverrides({ A: ovr(0, 0) });
    expect(canvasView.hasOverrides(defaults)).toBe(false);
  });
});
