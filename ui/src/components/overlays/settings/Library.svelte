<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { libraryStore } from '$lib/stores/library.svelte';
  import Icon from '../../Icon.svelte';
  import SectionHeader from './SectionHeader.svelte';

  onMount(() => {
    if (libraryStore.roots.length === 0) void libraryStore.refresh();
  });

  async function pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') await libraryStore.addRoot(picked);
  }
</script>

<SectionHeader title="Library" sub="Folders Superpanels indexes for the library." />

{#each libraryStore.roots as root (root)}
  <div class="row">
    <Icon name="folder" />
    <div class="mono" style:flex="1" style:font-size="12px" style:color="var(--text-2)">{root}</div>
    <button
      class="btn ghost sm"
      onclick={() => void libraryStore.removeRoot(root)}
      disabled={libraryStore.busyRoots}
    >
      Remove
    </button>
  </div>
{/each}

{#if libraryStore.roots.length === 0}
  <div style:padding="14px" style:font-size="12px" style:color="var(--text-3)">
    No folders configured yet.
  </div>
{/if}

<div class="flex" style:gap="8px" style:margin-top="16px">
  <button class="btn primary" onclick={() => void pickFolder()} disabled={libraryStore.busyRoots}>
    <Icon name="plus" size={12} />
    {libraryStore.busyRoots ? 'Adding…' : 'Add folder…'}
  </button>
  <button class="btn" onclick={() => void libraryStore.rescan()} disabled={libraryStore.loading}>
    <Icon name="refresh" size={12} />
    {libraryStore.loading ? 'Scanning…' : 'Rescan all'}
  </button>
</div>

<style>
  .row {
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 10px;
  }
</style>
