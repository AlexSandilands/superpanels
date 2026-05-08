<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';

  type Props = {
    x: number;
    y: number;
    isMaximized: boolean;
    alwaysOnTop: boolean;
    onClose: () => void;
    onAlwaysOnTopChange: (value: boolean) => void;
  };
  let { x, y, isMaximized, alwaysOnTop, onClose, onAlwaysOnTopChange }: Props = $props();

  let menuEl: HTMLDivElement | undefined = $state();
  // svelte-ignore state_referenced_locally
  let pos = $state({ left: x, top: y });

  async function safeCall(fn: () => Promise<unknown>) {
    try {
      await fn();
    } catch {
      // window may not be available outside Tauri (e.g. component preview); ignore.
    }
    onClose();
  }

  function minimise() {
    void safeCall(() => getCurrentWindow().minimize());
  }
  function toggleMaximise() {
    void safeCall(() => getCurrentWindow().toggleMaximize());
  }
  function startMove() {
    void safeCall(() => getCurrentWindow().startDragging());
  }
  function toggleAlwaysOnTop() {
    const next = !alwaysOnTop;
    onAlwaysOnTopChange(next);
    void safeCall(() => getCurrentWindow().setAlwaysOnTop(next));
  }
  function closeWindow() {
    void safeCall(() => getCurrentWindow().close());
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  onMount(() => {
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
  class="fixed inset-0"
  style:z-index="40"
  onclick={onClose}
  oncontextmenu={(e) => {
    e.preventDefault();
    onClose();
  }}
></div>
<div
  bind:this={menuEl}
  class="panel absolute"
  role="menu"
  style:left="{pos.left}px"
  style:top="{pos.top}px"
  style:min-width="200px"
  style:padding="4px"
  style:z-index="41"
>
  <button class="row" role="menuitem" onclick={minimise}>
    <span class="check"></span>
    <span class="label">Minimize</span>
  </button>
  <button class="row" role="menuitem" onclick={toggleMaximise}>
    <span class="check"></span>
    <span class="label">{isMaximized ? 'Restore' : 'Maximize'}</span>
  </button>
  <div class="sep"></div>
  <button class="row" role="menuitem" onclick={startMove}>
    <span class="check"></span>
    <span class="label">Move</span>
  </button>
  <button
    class="row"
    role="menuitemcheckbox"
    aria-checked={alwaysOnTop}
    onclick={toggleAlwaysOnTop}
  >
    <span class="check">{alwaysOnTop ? '✓' : ''}</span>
    <span class="label">Keep above others</span>
  </button>
  <div class="sep"></div>
  <button class="row danger" role="menuitem" onclick={closeWindow}>
    <span class="check"></span>
    <span class="label">Close</span>
    <span class="kbd" style:margin-left="auto">Alt+F4</span>
  </button>
</div>

<style>
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
  }
  .row:hover {
    background: var(--panel-2);
  }
  .row.danger:hover {
    background: color-mix(in oklab, var(--danger) 18%, transparent);
    color: var(--danger);
  }
  .check {
    width: 14px;
    text-align: center;
    color: var(--text-2);
    font-size: 11px;
    flex-shrink: 0;
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
