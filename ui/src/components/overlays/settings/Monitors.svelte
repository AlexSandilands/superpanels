<script lang="ts">
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import Icon from '../../Icon.svelte';
  import SectionHeader from './SectionHeader.svelte';

  async function redetect() {
    await monitorStore.refresh();
    toast.success(`Re-detected ${monitorStore.monitors.length} monitors`);
  }
</script>

<SectionHeader title="Monitors" sub="Detected displays and their physical sizes." />

{#each monitorStore.monitors as m (m.stable_id ?? m.name)}
  <div class="row">
    <div class="thumb">
      <div
        style:position="absolute"
        style:inset="3px"
        style:background="var(--bg-2)"
        style:border-radius="1px"
      ></div>
    </div>
    <div style:flex="1">
      <div style:font-size="13px" style:font-weight="600" style:display="flex" style:gap="8px">
        {m.name}
        {#if m.primary}<span class="chip active">PRIMARY</span>{/if}
      </div>
      <div class="mono" style:font-size="11px" style:color="var(--text-3)" style:margin-top="2px">
        {m.resolution[0]}×{m.resolution[1]}{m.refresh_hz ? ` @ ${m.refresh_hz}Hz` : ''}
        {#if m.physical_size_mm}
          · {m.physical_size_mm[0]}×{m.physical_size_mm[1]}mm
        {/if}
      </div>
      {#if !m.physical_size_mm}
        <div style:font-size="10px" style:color="var(--warn)" style:margin-top="4px">
          physical size missing — bezel math will be approximate
        </div>
      {:else}
        <div style:font-size="10px" style:color="var(--text-3)" style:margin-top="4px">
          size from compositor / config
        </div>
      {/if}
    </div>
  </div>
{/each}

{#if monitorStore.monitors.length === 0}
  <div style:padding="14px" style:font-size="12px" style:color="var(--text-3)">
    No monitors detected.
  </div>
{/if}

<div class="flex" style:gap="8px" style:margin-top="16px">
  <button class="btn" onclick={redetect} disabled={monitorStore.loading}>
    <Icon name="refresh" size={12} /> Re-detect (F5)
  </button>
</div>

<div style:margin-top="12px" style:font-size="11px" style:color="var(--text-3)">
  Editing a monitor's physical size from the GUI is not yet wired in this build — set
  <code class="mono">[[monitor.physical_size_mm]]</code> in the config file (Settings → General → Open
  config directory).
</div>

<style>
  .row {
    padding: 14px 0;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .thumb {
    width: 60px;
    height: 36px;
    border: 1.5px solid var(--line-2);
    border-radius: 3px;
    position: relative;
    flex-shrink: 0;
  }
</style>
