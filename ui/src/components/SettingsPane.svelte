<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { api, errorMessage } from '$lib/api';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { toast } from '$lib/stores/toast.svelte';

  let autostart = $state(false);
  let loading = $state(false);
  let firstRunChecked = $state(false);

  onMount(() => {
    void load();
    if (libraryStore.roots.length === 0) void libraryStore.refresh();
  });

  async function load() {
    loading = true;
    try {
      const r = await api.getAutostart();
      autostart = r.enabled;
      firstRunChecked = true;
    } catch (err) {
      toast.error('Could not read autostart state', errorMessage(err));
    } finally {
      loading = false;
    }
  }

  async function pickAndAddFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') await libraryStore.addRoot(picked);
  }

  async function toggle(enabled: boolean) {
    try {
      await api.setAutostart(enabled);
      autostart = enabled;
      toast.success(enabled ? 'Autostart enabled' : 'Autostart disabled');
    } catch (err) {
      toast.error('Could not change autostart', errorMessage(err));
    }
  }
</script>

<section class="flex flex-col gap-4">
  <h2 class="text-sm font-semibold text-slate-300">Settings</h2>

  <div class="rounded border border-slate-800 bg-slate-900/40 p-3">
    <label class="flex items-center justify-between gap-3 text-sm">
      <span>
        <span class="block font-medium text-slate-200">Start at login</span>
        <span class="text-xs text-slate-500">
          Writes <code>~/.config/autostart/superpanels.desktop</code>.
        </span>
      </span>
      <input
        type="checkbox"
        class="h-4 w-4"
        checked={autostart}
        disabled={loading}
        onchange={(e) => toggle(e.currentTarget.checked)}
      />
    </label>
  </div>

  <div class="flex flex-col gap-2 rounded border border-slate-800 bg-slate-900/40 p-3">
    <div class="flex items-center justify-between">
      <span class="block font-medium text-slate-200">Library folders</span>
      <button
        type="button"
        class="rounded border border-accent/60 bg-accent/10 px-2 py-1 text-xs text-accent hover:bg-accent/20"
        onclick={() => void pickAndAddFolder()}
        disabled={libraryStore.busyRoots}
      >
        {libraryStore.busyRoots ? 'Adding…' : 'Add folder…'}
      </button>
    </div>
    {#if libraryStore.roots.length === 0}
      <p class="text-xs text-slate-500">
        No folders yet. Add one to start scanning images into the library.
      </p>
    {:else}
      <ul class="flex flex-col gap-1 text-xs">
        {#each libraryStore.roots as root (root)}
          <li class="flex items-center justify-between gap-2 rounded bg-slate-950/60 px-2 py-1">
            <code class="truncate text-slate-200">{root}</code>
            <button
              type="button"
              class="shrink-0 rounded border border-slate-700 px-2 py-0.5 text-slate-300 hover:bg-rose-900/40 hover:text-rose-200"
              onclick={() => void libraryStore.removeRoot(root)}
              disabled={libraryStore.busyRoots}
            >
              Remove
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if !firstRunChecked}
    <p class="text-xs text-slate-500">Loading…</p>
  {/if}
</section>
