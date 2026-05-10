// Schedule-preemption sentinel + dirty-canvas snapshot (§4e.11.6).
//
// The daemon doesn't notify the GUI when a schedule fires; the GUI infers it
// by polling `current_state` and noticing `active_profile` changed *without*
// any user-driven action having claimed the switch. Without explicit claims
// from user flows, a successful manual switch races the polling refresh and
// briefly looks indistinguishable from a schedule fire — so user actions
// (`applyDraftProfile`, `saveActiveProfile`, `switchAndApply`) call
// `claimSwitchTo(targetName)` before issuing IPC, which advances the sentinel
// and drops any buffered snapshot. App.svelte's `$effect` only treats an
// active-name change as an external preemption when the sentinel disagrees.

import type { Profile } from '$lib/api';

let sentinel = $state<string | null>(null);
let snapshot: Profile | null = null;

export const preemption = {
  get sentinel() {
    return sentinel;
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
    sentinel = name;
    snapshot = null;
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
