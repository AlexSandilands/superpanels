// Schedule-preemption sentinel + dirty-canvas snapshot (§4e.11.6).
//
// The daemon doesn't notify the GUI when a schedule fires; the GUI infers it
// by polling `current_state` and noticing `active_profile` changed *without*
// any user-driven action having claimed the switch. User actions
// (`applyDraftProfile`, `saveActiveProfile`, `switchAndApply`) call
// `claimSwitchTo(targetName)` before issuing IPC; the claim stays *pending*
// until the runtime view reports the target, so a poll that raced the switch
// and still carries the old active name can't be misread as a schedule fire
// (it re-fired the sentinel before commands ran off the main thread, and the
// wider async window made it routine).

import type { Profile } from '$lib/api';

/** A user-driven switch that has been issued but not yet observed in the
 *  polled runtime state. `from` is the sentinel at claim time — polls
 *  reporting it are stale echoes, anything else is a genuine preemption. */
export type PendingClaim = { from: string | null; to: string | null };

let sentinel = $state<string | null>(null);
let pending = $state<PendingClaim | null>(null);
let snapshot: Profile | null = null;

export const preemption = {
  get sentinel() {
    return sentinel;
  },

  get pendingClaim(): PendingClaim | null {
    return pending;
  },

  /** Mark the sentinel as in-sync with the runtime view. Called from the
   *  $effect that observes `profileStore.activeName`. */
  setSentinel(name: string | null): void {
    sentinel = name;
  },

  /** Pre-claim an upcoming user-driven active-profile change so the polling
   *  refresh can't briefly mistake it for a schedule preemption. Also clears
   *  the buffered snapshot — once the user has decided to switch / save /
   *  apply, there's nothing left to "undo." */
  claimSwitchTo(name: string | null): void {
    pending = { from: sentinel, to: name };
    sentinel = name;
    snapshot = null;
  },

  /** The claimed switch has been observed (or superseded by a genuine
   *  external change) — stop filtering polls. */
  settleClaim(): void {
    pending = null;
  },

  /** Abandon an in-flight claim after its IPC failed, re-syncing the
   *  sentinel to the observed active name so the next poll isn't misread. */
  cancelClaim(current: string | null): void {
    pending = null;
    sentinel = current;
  },

  /** Replace the dirty-canvas snapshot used by the preemption-undo toast. */
  setSnapshot(next: Profile | null): void {
    snapshot = next;
  },

  /** Take ownership of the buffered snapshot (returning it and clearing the
   *  buffer). Called by the preemption $effect when it surfaces the toast. */
  consumeSnapshot(): Profile | null {
    const out = snapshot;
    snapshot = null;
    return out;
  },
};
