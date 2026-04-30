<script lang="ts">
  import { onMount } from 'svelte';
  import { libraryStore } from '$lib/stores/library.svelte';

  onMount(() => {
    void libraryStore.refresh();
  });
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-center justify-between">
    <h2 class="text-sm font-semibold text-slate-300">Library</h2>
    <button
      type="button"
      class="rounded border border-slate-700 px-2 py-0.5 text-xs hover:bg-slate-800"
      onclick={() => libraryStore.refresh()}
      disabled={libraryStore.loading}
    >
      {libraryStore.loading ? 'Scanning…' : 'Rescan'}
    </button>
  </div>

  {#if libraryStore.entries.length === 0}
    <p class="text-xs text-slate-500">Library is empty — add a root in Settings, then rescan.</p>
  {:else}
    <p class="text-xs text-slate-500">
      {libraryStore.entries.length} entries (Phase 4b adds thumbnails + filtering)
    </p>
  {/if}
</div>
