// Tracks whether the GUI can reach the `superpanels-daemon` socket.
// The bridge falls back to in-process for many IPC methods, so most calls
// silently succeed without a daemon — but Apply / Save / slideshow ops do
// not, and the `current_state` poll never reflects reality. This store
// drives the daemon-not-running banner so the user gets a clear signal
// instead of a half-working UI.
//
// Calls `invoke` directly (not `api.ts`) so we can register an error hook
// on `api.ts` without a circular import.

import { invoke } from '@tauri-apps/api/core';
import { errorMessage, isIpcError, setIpcErrorHook } from '$lib/api';

let connected = $state(true);
let starting = $state(false);
let lastError = $state<string | null>(null);

export const daemonStatus = {
  get connected() {
    return connected;
  },
  get starting() {
    return starting;
  },
  get lastError() {
    return lastError;
  },

  /** Probe the daemon socket once and update `connected`. */
  async probe(): Promise<void> {
    try {
      const r = await invoke<{ connected: boolean }>('daemon_status');
      connected = r.connected;
      if (r.connected) lastError = null;
    } catch (err) {
      connected = false;
      lastError = errorMessage(err);
    }
  },

  /** Mark the daemon as unreachable in response to an IPC error from any
   *  call site (e.g. apply_canvas / slideshow_*). Only flips when the
   *  error is the structural `DaemonUnreachable` variant — never on
   *  logical daemon rejections (` confused-deputy rules). */
  noteIpcError(err: unknown): void {
    if (isIpcError(err) && err.kind === 'DaemonUnreachable') {
      connected = false;
      lastError = err.message;
    }
  },

  /** Mark the daemon as reachable in response to a successful IPC. The
   *  bridge transparently falls back to in-process so most successes
   *  don't actually prove the daemon is up — only callers that know they
   *  hit the daemon (like `probe()`) should call this. */
  noteSuccess(): void {
    connected = true;
    lastError = null;
  },

  /** Spawn the daemon binary and re-probe a couple of times so the banner
   *  flips quickly on success. */
  async start(): Promise<void> {
    if (starting) return;
    starting = true;
    try {
      await invoke('start_daemon');
      // Daemon self-daemonises and binds the socket on startup; give it a
      // few attempts before declaring failure.
      for (let i = 0; i < 10; i += 1) {
        await delay(150);
        await this.probe();
        if (connected) break;
      }
    } catch (err) {
      lastError = errorMessage(err);
    } finally {
      starting = false;
    }
  },
};

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

setIpcErrorHook((err) => daemonStatus.noteIpcError(err));
