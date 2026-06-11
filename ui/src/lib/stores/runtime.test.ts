import { describe, it, expect } from 'vitest';
import { formatRelative, runtime } from './runtime.svelte';

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

describe('seedFromDaemon', () => {
  // The store is a module-level singleton, so these run as one sequence.
  it('seeds_an_empty_pill_without_flashing', () => {
    const flashBefore = runtime.flashKey;
    runtime.seedFromDaemon('kde', 1000);
    expect(runtime.last?.backend).toBe('kde');
    expect(runtime.last?.elapsedMs).toBeNull();
    expect(runtime.flashKey).toBe(flashBefore);
  });

  it('keeps_a_session_apply_over_an_older_daemon_stamp', () => {
    runtime.recordApply({ backend: 'session', elapsedMs: 5, monitorsSet: 3, at: 50_000 });
    runtime.seedFromDaemon('kde', 50_000 + 4000);
    expect(runtime.last?.backend).toBe('session');
  });

  it('takes_a_clearly_newer_daemon_apply', () => {
    runtime.seedFromDaemon('kde', 50_000 + 6000);
    expect(runtime.last?.backend).toBe('kde');
    expect(runtime.last?.at).toBe(56_000);
  });
});
