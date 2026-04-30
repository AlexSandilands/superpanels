// Profile store. Owns the profile list + active profile name.
// Uses Svelte 5 runes — module-state cells exposed through accessors.

import { api, errorMessage, type Profile } from '$lib/api';
import { toast } from './toast.svelte';

let profiles = $state<Profile[]>([]);
let activeName = $state<string | null>(null);
let loading = $state(false);

export const profileStore = {
  get profiles() {
    return profiles;
  },
  get activeName() {
    return activeName;
  },
  get loading() {
    return loading;
  },
  async refresh() {
    loading = true;
    try {
      const [list, runtime] = await Promise.all([api.listProfiles(), api.currentState()]);
      profiles = list;
      activeName = runtime.active_profile;
    } catch (err) {
      toast.error('Could not load profiles', errorMessage(err));
    } finally {
      loading = false;
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
      toast.success(`Deleted '${name}'`);
    } catch (err) {
      toast.error(`Failed to delete '${name}'`, errorMessage(err));
    }
  },
};
