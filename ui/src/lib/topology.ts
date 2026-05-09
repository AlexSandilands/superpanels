// Lightweight topology fingerprint mirror — the daemon owns the canonical
// hash, this is for UI-side defaults when authoring a brand new profile.
// The Rust hash is SHA-256; in the UI we just produce a deterministic stable
// string sorted-by-stable_id so equality checks against authored profiles
// work for the common case. The daemon recomputes on apply.

import type { Monitor } from './api';

export function TopologyFingerprintFor(monitors: Monitor[]): string {
  const entries = monitors.map((m) => `${m.stable_id ?? m.name}:${m.rotation}`).sort();
  return entries.join('|');
}
