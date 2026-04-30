<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Sidebar from './components/Sidebar.svelte';
  import MonitorCanvas from './components/MonitorCanvas.svelte';
  import ProfileList from './components/ProfileList.svelte';
  import LibraryPane from './components/LibraryPane.svelte';
  import SettingsPane from './components/SettingsPane.svelte';
  import Toast from './components/Toast.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';

  type Tab = 'profiles' | 'library' | 'settings';
  let tab = $state<Tab>('profiles');

  onMount(() => {
    void monitorStore.refresh();
    void profileStore.refresh();

    let unlisten: UnlistenFn | undefined;
    void listen('tray://open-settings', () => {
      tab = 'settings';
    }).then((fn) => {
      unlisten = fn;
    });

    // Re-poll runtime state every few seconds so the active-profile badge in
    // the profile list stays in sync with daemon-side changes (slideshow,
    // schedules, tray-driven applies).
    const interval = window.setInterval(() => {
      void profileStore.refresh();
    }, 5000);

    return () => {
      unlisten?.();
      window.clearInterval(interval);
    };
  });
</script>

<div class="flex h-full w-full">
  <Sidebar bind:tab />

  <main class="flex flex-1 flex-col gap-3 p-4">
    <section class="h-1/2 min-h-[200px]">
      <MonitorCanvas monitors={monitorStore.monitors} />
    </section>

    <section class="flex-1 overflow-auto rounded border border-slate-800 bg-slate-900/30 p-3">
      {#if tab === 'profiles'}
        <ProfileList />
      {:else if tab === 'library'}
        <LibraryPane />
      {:else}
        <SettingsPane />
      {/if}
    </section>
  </main>
</div>

<Toast />
