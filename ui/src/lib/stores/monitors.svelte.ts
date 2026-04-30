// Monitor layout store. Owns the detected list and a re-detect action.

import { api, errorMessage, type Monitor } from '$lib/api';
import { toast } from './toast.svelte';

let monitors = $state<Monitor[]>([]);
let loading = $state(false);

export const monitorStore = {
  get monitors() {
    return monitors;
  },
  get loading() {
    return loading;
  },
  async refresh() {
    loading = true;
    try {
      monitors = await api.detectMonitors();
    } catch (err) {
      toast.error('Could not detect monitors', errorMessage(err));
      monitors = [];
    } finally {
      loading = false;
    }
  },
};
