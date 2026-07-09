<script lang="ts">
  // Right-click z-order menu for a composite-mode layer. Portalled to `body`
  // (see Select.svelte) — the canvas sits alongside `.panel` docks whose
  // backdrop-filter traps fixed descendants otherwise.
  import { portal } from '$lib/portal';
  import { canvasLayers } from '$lib/stores/canvas-layers.svelte';

  type Props = {
    layerId: string;
    x: number;
    y: number;
    onClose: () => void;
  };
  let { layerId, x, y, onClose }: Props = $props();

  let menuEl: HTMLDivElement | undefined = $state();
  // svelte-ignore state_referenced_locally
  let pos = $state({ left: x, top: y });

  const idx = $derived(canvasLayers.list.findIndex((l) => l.id === layerId));
  const atTop = $derived(idx === canvasLayers.list.length - 1);
  const atBottom = $derived(idx === 0);

  function bringToFront() {
    canvasLayers.bringToFront(layerId);
    onClose();
  }
  function bringForward() {
    canvasLayers.bringForward(layerId);
    onClose();
  }
  function sendBackward() {
    canvasLayers.sendBackward(layerId);
    onClose();
  }
  function sendToBack() {
    canvasLayers.sendToBack(layerId);
    onClose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  $effect(() => {
    if (!menuEl) return;
    const rect = menuEl.getBoundingClientRect();
    const margin = 6;
    const left = Math.min(x, window.innerWidth - rect.width - margin);
    const top = Math.min(y, window.innerHeight - rect.height - margin);
    pos = { left: Math.max(margin, left), top: Math.max(margin, top) };
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  use:portal
  class="fixed inset-0"
  style:z-index="40"
  onclick={onClose}
  oncontextmenu={(e) => {
    e.preventDefault();
    onClose();
  }}
></div>
<div
  use:portal
  bind:this={menuEl}
  class="panel menu"
  role="menu"
  aria-label="Layer order"
  style:left="{pos.left}px"
  style:top="{pos.top}px"
>
  <button class="row" role="menuitem" disabled={atTop} onclick={bringToFront}>
    <span class="label">Bring to Front</span>
  </button>
  <button class="row" role="menuitem" disabled={atTop} onclick={bringForward}>
    <span class="label">Bring Forward</span>
  </button>
  <div class="sep"></div>
  <button class="row" role="menuitem" disabled={atBottom} onclick={sendBackward}>
    <span class="label">Send Backward</span>
  </button>
  <button class="row" role="menuitem" disabled={atBottom} onclick={sendToBack}>
    <span class="label">Send to Back</span>
  </button>
</div>

<style>
  .menu {
    position: fixed;
    padding: 4px;
    min-width: 170px;
    z-index: 41;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px 6px 6px;
    border-radius: 5px;
    border: none;
    background: transparent;
    color: inherit;
    text-align: left;
    font-size: 12px;
    font-family: inherit;
    cursor: default;
  }
  .row:hover:not(:disabled) {
    background: var(--panel-2);
  }
  .row:disabled {
    color: var(--text-3);
  }
  .label {
    flex: 1;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 4px 2px;
  }
</style>
