<script lang="ts">
  import { ui, ACCENT_OPTIONS, type Density, type Theme } from '$lib/stores/ui.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';

  const themes: Theme[] = ['auto', 'light', 'dark'];
  const densities: Density[] = ['compact', 'regular', 'spacious'];
</script>

<SectionHeader title="Appearance" />

<SettingRow label="Theme">
  <div class="seg">
    {#each themes as t (t)}
      <button class:seg-active={ui.theme === t} onclick={() => ui.set({ theme: t })}>
        {t}
      </button>
    {/each}
  </div>
</SettingRow>

<SettingRow label="Density" sub="Controls dock and panel padding throughout the app.">
  <div class="seg">
    {#each densities as d (d)}
      <button class:seg-active={ui.density === d} onclick={() => ui.set({ density: d })}>
        {d}
      </button>
    {/each}
  </div>
</SettingRow>

<SettingRow
  label="Accent"
  sub="Used for selection, primary action, dimension callouts. Follows system on KDE Plasma 6 when enabled."
>
  <div class="flex items-center" style:gap="6px">
    {#each ACCENT_OPTIONS as c (c)}
      <button
        class="swatch"
        style:background={c}
        style:box-shadow={ui.accent === c ? `0 0 0 2px white, 0 0 0 4px ${c}` : 'none'}
        onclick={() => ui.set({ accent: c })}
        aria-label="Pick accent {c}"
      ></button>
    {/each}
  </div>
</SettingRow>

<SettingRow label="Follow KDE system accent">
  <Toggle value={ui.followSystemAccent} onChange={(v) => ui.set({ followSystemAccent: v })} />
</SettingRow>

<SettingRow label="Window blur" sub="Disable on low-power machines.">
  <Toggle value={ui.windowBlur} onChange={(v) => ui.set({ windowBlur: v })} />
</SettingRow>

<SettingRow
  label="Always show bezel mm dimensions"
  sub="When off, lines appear only on hover or selection."
>
  <Toggle value={ui.dimsAlways} onChange={(v) => ui.set({ dimsAlways: v })} />
</SettingRow>

<style>
  .seg {
    display: inline-flex;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .seg button {
    border: none;
    height: 28px;
    padding: 0 14px;
    background: transparent;
    color: var(--text-2);
    font-size: 12px;
    text-transform: capitalize;
  }
  .seg-active {
    background: var(--accent) !important;
    color: oklch(0.16 0.01 250) !important;
  }
  .swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: none;
    padding: 0;
  }
</style>
