<script lang="ts">
  // Top-left action buttons (snap-to-cover, aspect lock) + top-right status
  // badges (zoom, dim) overlaid on the monitor preview canvas. Pulled out of
  // `MonitorCanvas.svelte` to keep that file under the Svelte size cap.

  type Props = {
    showActionButtons: boolean;
    aspectLock: boolean;
    onSnapToCover: () => void;
    onToggleAspectLock: () => void;
    zoom: number;
    dim: boolean;
  };

  let { showActionButtons, aspectLock, onSnapToCover, onToggleAspectLock, zoom, dim }: Props =
    $props();
</script>

<div
  class="pointer-events-none absolute right-2 top-2 flex flex-col items-end gap-1 text-[10px] uppercase tracking-wide text-slate-400"
>
  <span class="rounded bg-slate-900/70 px-1.5 py-0.5">
    zoom {(zoom * 100).toFixed(0)}%
  </span>
  <span class="rounded bg-slate-900/70 px-1.5 py-0.5">
    dim {dim ? 'on' : 'off'} · D
  </span>
</div>

{#if showActionButtons}
  <div class="absolute left-2 top-2 flex gap-1 text-[10px] uppercase text-slate-200">
    <button
      type="button"
      class="rounded bg-slate-900/80 px-2 py-1 hover:bg-slate-800"
      title="Snap the image to cover all monitors at the smallest aspect-preserving scale."
      onclick={onSnapToCover}
    >
      Cover all monitors
    </button>
    <button
      type="button"
      class="rounded px-2 py-1 hover:bg-slate-800"
      class:bg-slate-900={!aspectLock}
      class:bg-accent={aspectLock}
      class:text-slate-900={aspectLock}
      title={aspectLock
        ? 'Aspect lock on — corner resize keeps the natural image ratio.'
        : 'Aspect lock off — corner resize stretches each axis independently.'}
      onclick={onToggleAspectLock}
    >
      Lock aspect{aspectLock ? ' · on' : ' · off'}
    </button>
  </div>
{/if}
