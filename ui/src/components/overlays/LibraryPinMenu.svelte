<script lang="ts">
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { stableId } from '$lib/canvas/preview-layout';

  type Props = {
    onPin: (monitorId: string) => void;
  };
  let { onPin }: Props = $props();
</script>

<div class="pin-pop">
  <div class="section-label" style:margin-bottom="4px">Pin to…</div>
  {#if monitorStore.monitors.length === 0}
    <div style:font-size="11px" style:color="var(--text-3)">No monitors detected.</div>
  {:else}
    {#each monitorStore.monitors as m (stableId(m))}
      <button
        class="pin-row"
        onclick={(ev) => {
          ev.stopPropagation();
          onPin(stableId(m));
        }}
      >
        <span class="mono" style:font-size="11px">{m.name}</span>
        <span
          class="mono"
          style:font-size="10px"
          style:color="var(--text-3)"
          style:margin-left="auto"
        >
          {m.resolution[0]}×{m.resolution[1]}
        </span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .section-label {
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-transform: uppercase;
    margin-bottom: 8px;
  }
  .pin-pop {
    margin-top: 6px;
    padding: 8px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .pin-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border: none;
    background: transparent;
    color: inherit;
    border-radius: 4px;
    text-align: left;
  }
  .pin-row:hover {
    background: var(--panel-2);
  }
</style>
