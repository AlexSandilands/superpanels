import { describe, it, expect } from 'vitest';
import { formatRelative } from './runtime.svelte';

describe('formatRelative', () => {
  it('returns_just_now_below_1500_ms', () => {
    expect(formatRelative(0)).toBe('just now');
    expect(formatRelative(1499)).toBe('just now');
  });

  it('rolls_to_seconds_at_1500_ms', () => {
    expect(formatRelative(1500)).toBe('2s ago');
  });

  it('rolls_to_minutes_at_one_minute', () => {
    expect(formatRelative(59 * 1000)).toBe('59s ago');
    expect(formatRelative(60 * 1000)).toBe('1m ago');
  });

  it('rolls_to_hours_at_60_minutes', () => {
    expect(formatRelative(59 * 60 * 1000)).toBe('59m ago');
    expect(formatRelative(60 * 60 * 1000)).toBe('1h ago');
  });
});
