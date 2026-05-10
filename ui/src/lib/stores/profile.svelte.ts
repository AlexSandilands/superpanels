// Profile store. Owns the profile list, the active profile name, and the
// per-edit "draft" buffer that the canvas, docks, and settings panes mutate.
// `Save` (§4e.11) commits the draft via `save_profile`; `Revert` re-pulls
// the active profile's persisted state.

import { api, errorMessage, type Profile } from '$lib/api';
import { defaultSlideshowConfig } from '$lib/types/profile-helpers';
import { toast } from './toast.svelte';

let profiles = $state<Profile[]>([]);
let activeName = $state<string | null>(null);
let selectedName = $state<string | null>(null);
let draft = $state<Profile | null>(null);
let dirty = $state(false);
let loading = $state(false);
let saving = $state(false);

export const profileStore = {
  get profiles() {
    return profiles;
  },
  get activeName() {
    return activeName;
  },
  get activeProfile(): Profile | null {
    return activeName ? (profiles.find((p) => p.name === activeName) ?? null) : null;
  },
  get selectedName() {
    return selectedName;
  },
  get draft() {
    return draft;
  },
  get dirty() {
    return dirty;
  },
  get loading() {
    return loading;
  },
  get saving() {
    return saving;
  },

  async refresh() {
    loading = true;
    try {
      const [resp, runtime] = await Promise.all([api.listProfiles(), api.currentState()]);
      const list = resp.profiles;
      profiles = list;
      activeName = runtime.active_profile;
      // Don't clobber an in-progress edit. If the user has unsaved changes
      // (including a brand-new untitled profile that hasn't been saved yet),
      // leave the editor alone — refresh is just keeping the *list* current.
      if (dirty) return;
      // Re-select the same profile (or first one) so the editor stays consistent.
      const keep = selectedName && list.some((p) => p.name === selectedName) ? selectedName : null;
      const fallback = keep ?? activeName ?? list[0]?.name ?? null;
      if (fallback !== selectedName || draft === null) {
        select(fallback);
      }
    } catch (err) {
      toast.error('Could not load profiles', errorMessage(err));
    } finally {
      loading = false;
    }
  },

  select(name: string | null) {
    select(name);
  },

  /** Mutate the draft in place; sets `dirty` if the change matters. */
  patchDraft(mutator: (d: Profile) => void) {
    if (!draft) return;
    mutator(draft);
    dirty = true;
  },

  /** Replace the whole draft (e.g., switching body type). */
  replaceDraft(next: Profile) {
    draft = next;
    dirty = true;
  },

  /** Discard local edits and re-snap to the saved profile. */
  revertDraft() {
    if (!selectedName) return;
    const saved = profiles.find((p) => p.name === selectedName) ?? null;
    draft = saved ? snapshotClone(saved) : null;
    dirty = false;
  },

  /** Re-pull the *active* profile (§4e.11.4 Revert). The Revert button in the
   *  TitleBar uses this rather than `revertDraft` because the canvas is
   *  conceptually authored against the active profile, not the manager-pane
   *  selection. Returns the active profile snapshot the caller can use to
   *  re-seed canvas-view + image-transform stores. */
  revertToActive(): Profile | null {
    if (!activeName) return null;
    const saved = profiles.find((p) => p.name === activeName) ?? null;
    if (!saved) return null;
    selectedName = activeName;
    draft = snapshotClone(saved);
    dirty = false;
    return saved;
  },

  /** Force-clear the dirty flag (e.g. after a successful Save commits the
   *  active profile via `save_profile`). The caller is responsible for
   *  ensuring the in-memory profile list reflects the committed state. */
  clearDirty(): void {
    dirty = false;
  },

  newProfile() {
    const now = new Date().toISOString();
    const base: Profile = {
      name: uniqueName('untitled', profiles),
      body: {
        type: 'span',
        source: { type: 'single', path: '' },
        image_rect_mm: { x_mm: 0, y_mm: 0, w_mm: 0, h_mm: 0 },
      },
      monitor_state: {},
      topology: '',
      colour: 'slate',
      description: null,
      created_at: now,
      updated_at: now,
      last_applied_at: null,
      backend_override: null,
    };
    draft = base;
    selectedName = null;
    dirty = true;
  },

  async save(): Promise<boolean> {
    if (!draft) return false;
    saving = true;
    try {
      await api.saveProfile(draft);
      const newName = draft.name;
      // Replace or append in the in-memory list.
      const i = profiles.findIndex((p) => p.name === newName);
      if (i >= 0) {
        profiles[i] = snapshotClone(draft);
      } else {
        profiles = [...profiles, snapshotClone(draft)];
      }
      selectedName = newName;
      dirty = false;
      toast.success(`Saved '${newName}'`);
      return true;
    } catch (err) {
      toast.error('Failed to save profile', errorMessage(err));
      return false;
    } finally {
      saving = false;
    }
  },

  async apply(name: string) {
    try {
      const report = await api.applyProfile(name);
      activeName = name;
      const m = report.monitors_set ?? 0;
      toast.success(
        `Applied '${name}'`,
        `${m} monitor${m === 1 ? '' : 's'}${report.backend ? ` via ${report.backend}` : ''}`,
      );
    } catch (err) {
      toast.error(`Failed to apply '${name}'`, errorMessage(err));
    }
  },

  async delete(name: string) {
    try {
      await api.deleteProfile(name);
      profiles = profiles.filter((p) => p.name !== name);
      if (selectedName === name) select(profiles[0]?.name ?? null);
      toast.success(`Deleted '${name}'`);
    } catch (err) {
      toast.error(`Failed to delete '${name}'`, errorMessage(err));
    }
  },
};

function select(name: string | null) {
  selectedName = name;
  const saved = name ? (profiles.find((p) => p.name === name) ?? null) : null;
  draft = saved ? snapshotClone(saved) : null;
  dirty = false;
}

// Svelte 5 wraps `$state` values in a Proxy. `structuredClone` rejects those
// with `DataCloneError`; `$state.snapshot()` already returns a plain, deep,
// non-reactive copy, which is what we want for the editable draft.
function snapshotClone<T>(value: T): T {
  return $state.snapshot(value) as T;
}

function uniqueName(base: string, existing: Profile[]): string {
  if (!existing.some((p) => p.name === base)) return base;
  for (let i = 2; i < 1000; i += 1) {
    const candidate = `${base}-${i}`;
    if (!existing.some((p) => p.name === candidate)) return candidate;
  }
  return `${base}-${Date.now()}`;
}

// Re-export so consumers can build sensible defaults without re-importing
// from the types module.
export { defaultSlideshowConfig };
