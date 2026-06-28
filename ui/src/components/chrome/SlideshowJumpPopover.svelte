<script lang="ts">
  // Quick-jump grid for the slideshow: a thumbnail of every set image, click to
  // jump straight to it instead of stepping through with prev/next.
  import LibraryThumb from '../overlays/LibraryThumb.svelte';

  type Props = {
    images: string[];
    current: string | null;
    onJump: (path: string) => void;
    onClose: () => void;
  };
  let { images, current, onJump, onClose }: Props = $props();

  function fileName(path: string): string {
    const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return i >= 0 ? path.slice(i + 1) : path;
  }

  function pick(path: string) {
    onJump(path);
    onClose();
  }
</script>

<div class="jump-backdrop" role="presentation" onclick={onClose}></div>
<div class="jump-pop panel" role="dialog" aria-label="Jump to slideshow image">
  <div class="jump-head">
    <span class="jump-title">Jump to image</span>
    <span class="mono jump-count">{images.length}</span>
  </div>
  <div class="jump-grid">
    {#each images as path (path)}
      <button
        class="jump-cell"
        class:current={path === current}
        title={fileName(path)}
        onclick={() => pick(path)}
      >
        <LibraryThumb {path} alt={fileName(path)} />
      </button>
    {/each}
  </div>
</div>

<style>
  .jump-backdrop {
    position: fixed;
    inset: 0;
    z-index: 18;
  }
  .jump-pop {
    position: absolute;
    right: 0;
    bottom: calc(100% + 8px);
    z-index: 19;
    width: 320px;
    max-height: 320px;
    display: flex;
    flex-direction: column;
    padding: 8px;
    gap: 8px;
  }
  .jump-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 2px;
  }
  .jump-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-2);
  }
  .jump-count {
    font-size: 10px;
    color: var(--text-3);
  }
  .jump-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
    overflow-y: auto;
    padding-right: 2px;
  }
  .jump-cell {
    aspect-ratio: 16 / 10;
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: var(--bg-2);
    padding: 0;
    cursor: pointer;
    transition:
      border-color 80ms,
      box-shadow 80ms;
  }
  .jump-cell:hover {
    border-color: var(--line-2);
  }
  .jump-cell.current {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
</style>
