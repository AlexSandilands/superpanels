<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Icon from '../Icon.svelte';

  async function minimise() {
    try {
      await getCurrentWindow().minimize();
    } catch {
      // pre-Tauri or no window context — ignore.
    }
  }
  async function toggleMaximise() {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch {
      // ignore
    }
  }
  async function close() {
    try {
      await getCurrentWindow().close();
    } catch {
      // ignore
    }
  }
</script>

<div class="flex gap-1 pl-0.5" data-tauri-drag-region="false">
  <button class="wc" onclick={minimise} title="Minimize" data-tauri-drag-region="false">
    <Icon name="win-min" size={10} />
  </button>
  <button class="wc" onclick={toggleMaximise} title="Maximize" data-tauri-drag-region="false">
    <Icon name="win-max" size={10} />
  </button>
  <button class="wc wc-close" onclick={close} title="Close" data-tauri-drag-region="false">
    <Icon name="win-close" size={10} />
  </button>
</div>

<style>
  .wc {
    appearance: none;
    border: 1px solid var(--line);
    background: var(--panel-2);
    color: var(--text-2);
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition:
      background 80ms,
      color 80ms,
      border-color 80ms;
  }
  .wc:hover {
    background: var(--line);
    color: var(--text);
    border-color: var(--line-2);
  }
  .wc-close:hover {
    background: var(--danger);
    color: white;
    border-color: var(--danger);
  }
</style>
