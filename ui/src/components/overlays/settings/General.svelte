<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import { toast } from '$lib/stores/toast.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';

  let autostart = $state(false);
  let autostartLoaded = $state(false);

  onMount(async () => {
    try {
      const r = await api.getAutostart();
      autostart = r.enabled;
      autostartLoaded = true;
    } catch (err) {
      toast.error('Could not read autostart state', errorMessage(err));
    }
  });

  async function setAutostart(v: boolean) {
    try {
      await api.setAutostart(v);
      autostart = v;
      toast.success(v ? 'Autostart enabled' : 'Autostart disabled');
    } catch (err) {
      toast.error('Could not change autostart', errorMessage(err));
    }
  }
</script>

<SectionHeader title="General" sub="App behaviour and notifications." />
<SettingRow label="Autostart on login" sub="Writes ~/.config/autostart/superpanels.desktop">
  <Toggle value={autostart} onChange={setAutostart} />
</SettingRow>
<SettingRow
  label="Run in tray when window closes"
  sub="Closing the window keeps the daemon active in the system tray."
>
  <Toggle value={ui.trayRun} onChange={(v) => ui.set({ trayRun: v })} />
</SettingRow>
<SettingRow label="Notifications" sub="Errors are always written to the log regardless">
  <select
    class="field ui"
    value={ui.notify}
    onchange={(e) =>
      ui.set({
        notify: (e.currentTarget as HTMLSelectElement).value as 'off' | 'errors only' | 'all',
      })}
  >
    <option value="off">off</option>
    <option value="errors only">errors only</option>
    <option value="all">all</option>
  </select>
</SettingRow>
<SettingRow label="Reduced motion" sub="Mirrors prefers-reduced-motion when set to system.">
  <select
    class="field ui"
    value={ui.motion}
    onchange={(e) =>
      ui.set({ motion: (e.currentTarget as HTMLSelectElement).value as 'system' | 'on' | 'off' })}
  >
    <option value="system">system</option>
    <option value="on">on</option>
    <option value="off">off</option>
  </select>
</SettingRow>
<SettingRow label="Locale" sub="Only English ships in v1.">
  <select
    class="field ui"
    value={ui.locale}
    onchange={(e) => ui.set({ locale: (e.currentTarget as HTMLSelectElement).value })}
  >
    <option value="en-US (system)">en-US (system)</option>
  </select>
</SettingRow>

{#if !autostartLoaded}
  <div style:font-size="11px" style:color="var(--text-3)" style:margin-top="8px">Loading…</div>
{/if}

<div class="flex" style:gap="8px" style:flex-wrap="wrap" style:margin-top="24px">
  <button class="btn" onclick={() => toast.info('Open log file', 'not yet wired in this build')}>
    Open log file
  </button>
  <button
    class="btn"
    onclick={() => toast.info('Open config directory', 'not yet wired in this build')}
  >
    Open config directory
  </button>
  <button class="btn" onclick={() => toast.info('Open library DB', 'not yet wired in this build')}>
    Open library DB
  </button>
</div>
