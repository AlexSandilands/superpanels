<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import { toast } from '$lib/stores/toast.svelte';

  let autostart = $state(false);
  let loading = $state(false);
  let firstRunChecked = $state(false);

  onMount(() => {
    void load();
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

  {#if !firstRunChecked}
    <p class="text-xs text-slate-500">Loading…</p>
  {/if}
</section>
