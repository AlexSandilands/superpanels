import { beforeEach, describe, expect, it } from 'vitest';
import type { Profile } from '$lib/api';
import { preemption } from './preemption.svelte';

function reset(): void {
  preemption.cancelClaim(null);
  preemption.setSentinel(null);
  preemption.setSnapshot(null);
}

describe('preemption claim lifecycle', () => {
  beforeEach(reset);

  it('claimSwitchTo records the from/to pair and advances the sentinel', () => {
    preemption.setSentinel('old');
    preemption.claimSwitchTo('new');
    expect(preemption.sentinel).toBe('new');
    expect(preemption.pendingClaim).toEqual({ from: 'old', to: 'new' });
  });

  it('claimSwitchTo drops a buffered snapshot — nothing left to undo', () => {
    preemption.setSnapshot({ name: 'p' } as Profile);
    preemption.claimSwitchTo('new');
    expect(preemption.consumeSnapshot()).toBeNull();
  });

  it('settleClaim clears the pending claim and keeps the sentinel', () => {
    preemption.claimSwitchTo('new');
    preemption.settleClaim();
    expect(preemption.pendingClaim).toBeNull();
    expect(preemption.sentinel).toBe('new');
  });

  it('cancelClaim re-syncs the sentinel to the observed active name', () => {
    preemption.setSentinel('old');
    preemption.claimSwitchTo('new');
    preemption.cancelClaim('old');
    expect(preemption.pendingClaim).toBeNull();
    expect(preemption.sentinel).toBe('old');
  });

  it('consumeSnapshot hands over the buffer exactly once', () => {
    const snap = { name: 'p' } as Profile;
    preemption.setSnapshot(snap);
    expect(preemption.consumeSnapshot()).toBe(snap);
    expect(preemption.consumeSnapshot()).toBeNull();
  });
});
