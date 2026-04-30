// Library store. Phase 3 only exposes a flat `entries` list — filtering and
// thumbnails arrive in phase 4b but the shape is stable.

import { api, errorMessage } from '$lib/api';
import type { LibraryFilter } from '$lib/types/LibraryFilter';
import { toast } from './toast.svelte';

let entries = $state<unknown[]>([]);
let loading = $state(false);

export const libraryStore = {
  get entries() {
    return entries;
  },
  get loading() {
    return loading;
  },
  async refresh(filter: LibraryFilter = {}) {
    loading = true;
    try {
      entries = await api.libraryList(filter);
    } catch (err) {
      toast.error('Could not load library', errorMessage(err));
      entries = [];
    } finally {
      loading = false;
    }
  },
};
