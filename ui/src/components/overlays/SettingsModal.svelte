<script lang="ts">
  import Backdrop from '../chrome/Backdrop.svelte';
  import About from './settings/About.svelte';
  import Appearance from './settings/Appearance.svelte';
  import Backends from './settings/Backends.svelte';
  import General from './settings/General.svelte';
  import Library from './settings/Library.svelte';
  import Monitors from './settings/Monitors.svelte';
  import Schedules from './settings/Schedules.svelte';
  import Shortcuts from './settings/Shortcuts.svelte';

  type Section =
    | 'general'
    | 'appearance'
    | 'monitors'
    | 'library'
    | 'backends'
    | 'schedules'
    | 'shortcuts'
    | 'about';

  type Props = { initialSection?: Section; onClose: () => void };
  let { initialSection = 'general', onClose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let section = $state<Section>(initialSection);

  const tabs: Array<{ id: Section; label: string }> = [
    { id: 'general', label: 'General' },
    { id: 'appearance', label: 'Appearance' },
    { id: 'monitors', label: 'Monitors' },
    { id: 'library', label: 'Library' },
    { id: 'backends', label: 'Backends' },
    { id: 'schedules', label: 'Schedules' },
    { id: 'shortcuts', label: 'Shortcuts' },
    { id: 'about', label: 'About' },
  ];
</script>

<Backdrop {onClose}>
  <div
    class="panel flex overflow-hidden"
    style:width="min(880px, 92vw)"
    style:height="min(620px, 84vh)"
  >
    <div
      style:width="180px"
      style:border-right="1px solid var(--line)"
      style:padding="8px"
      style:background="var(--panel-2)"
    >
      <div style:padding="8px 10px 12px" style:font-size="13px" style:font-weight="600">
        Settings
      </div>
      {#each tabs as t (t.id)}
        <button class="tab" class:tab-active={section === t.id} onclick={() => (section = t.id)}>
          {t.label}
        </button>
      {/each}
    </div>
    <div class="scroll" style:flex="1" style:padding="28px" style:position="relative">
      <button
        class="btn ghost icon"
        style:position="absolute"
        style:top="14px"
        style:right="14px"
        onclick={onClose}
        aria-label="Close"
      >
        ×
      </button>
      {#if section === 'general'}
        <General />
      {:else if section === 'appearance'}
        <Appearance />
      {:else if section === 'monitors'}
        <Monitors />
      {:else if section === 'library'}
        <Library />
      {:else if section === 'backends'}
        <Backends />
      {:else if section === 'schedules'}
        <Schedules />
      {:else if section === 'shortcuts'}
        <Shortcuts />
      {:else if section === 'about'}
        <About />
      {/if}
    </div>
  </div>
</Backdrop>

<style>
  .tab {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    border-radius: 5px;
    font-size: 12px;
    background: transparent;
    color: var(--text-2);
    border: none;
    font-weight: 400;
  }
  .tab:hover {
    background: var(--panel);
  }
  .tab-active {
    background: color-mix(in oklab, var(--accent) 16%, transparent) !important;
    color: var(--accent) !important;
    font-weight: 600;
  }
</style>
