<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import { toast } from '$lib/stores/toast.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import Select from '../../widgets/Select.svelte';
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

  async function openConfigFile() {
    try {
      await api.openConfigFile();
    } catch (err) {
      toast.error('Could not open config file', errorMessage(err));
    }
  }
</script>

<SectionHeader title="General" sub="App behaviour and notifications." />
<SettingRow label="Autostart on login" sub="Start Superpanels in the tray when you log in">
  <Toggle value={autostart} onChange={setAutostart} />
</SettingRow>
<SettingRow
  label="Run in tray when window closes"
  sub="Closing the window keeps the daemon active in the system tray."
>
  <Toggle value={ui.trayRun} onChange={(v) => ui.set({ trayRun: v })} />
</SettingRow>
<SettingRow label="Notifications" sub="Errors are always written to the log regardless">
  <Select
    value={ui.notify}
    options={[
      { value: 'off', label: 'off' },
      { value: 'errors only', label: 'errors only' },
      { value: 'all', label: 'all' },
    ]}
    onChange={(v) => ui.set({ notify: v as 'off' | 'errors only' | 'all' })}
    minWidth={140}
  />
</SettingRow>
<SettingRow label="Reduced motion" sub="Mirrors prefers-reduced-motion when set to system.">
  <Select
    value={ui.motion}
    options={[
      { value: 'system', label: 'system' },
      { value: 'on', label: 'on' },
      { value: 'off', label: 'off' },
    ]}
    onChange={(v) => ui.set({ motion: v as 'system' | 'on' | 'off' })}
    minWidth={140}
  />
</SettingRow>
<SettingRow label="Locale" sub="Only English ships in v1.">
  <Select
    value={ui.locale}
    options={[{ value: 'en-US (system)', label: 'en-US (system)' }]}
    onChange={(v) => ui.set({ locale: v })}
    minWidth={180}
  />
</SettingRow>

{#if !autostartLoaded}
  <div style:font-size="11px" style:color="var(--text-3)" style:margin-top="8px">Loading…</div>
{/if}

<div class="flex" style:gap="8px" style:flex-wrap="wrap" style:margin-top="24px">
  <button class="btn" onclick={() => toast.info('Open log file', 'not yet wired in this build')}>
    Open log file
  </button>
  <button class="btn" onclick={openConfigFile}>Open config file</button>
  <button class="btn" onclick={() => toast.info('Open library DB', 'not yet wired in this build')}>
    Open library DB
  </button>
</div>
