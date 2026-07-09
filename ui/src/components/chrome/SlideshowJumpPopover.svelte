<script lang="ts">
  // Quick-jump grid for the slideshow: a thumbnail of every set image, click to
  // jump straight to it instead of stepping through with prev/next. Portaled to
  // body (the `.panel` dock clips fixed descendants) so the backdrop covers the
  // viewport and an outside click closes it — mirrors SlideshowSettingsPopover.
  import { portal } from '$lib/portal';
  import LibraryThumb from '../overlays/LibraryThumb.svelte';

  type Props = {
    /** Element the popover hangs above; also fixes its right edge. */
    anchor: HTMLElement;
    images: string[];
    current: string | null;
    onJump: (path: string) => void;
    onClose: () => void;
  };
  let { anchor, images, current, onJump, onClose }: Props = $props();

  // Position is computed once at open — the popover remounts per open.
  // svelte-ignore state_referenced_locally
  const rect = anchor.getBoundingClientRect();
  const right = window.innerWidth - rect.right;
  const bottom = window.innerHeight - rect.top + 10;

  function fileName(path: string): string {
    const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return i >= 0 ? path.slice(i + 1) : path;
  }

  function pick(path: string) {
    onJump(path);
    onClose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div use:portal class="fixed inset-0" style:z-index="40" onclick={onClose}></div>
<div
  use:portal
  class="panel jump-pop"
  role="dialog"
  aria-label="Jump to slideshow image"
  style:right="{right}px"
  style:bottom="{bottom}px"
>
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
  .jump-pop {
    position: fixed;
    z-index: 41;
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
    /* Once the popover hits its max-height this grid gets a definite height, and
       WebKit then divides it across the implicit `auto` rows instead of sizing
       them from the cells' aspect-ratio — rows collapse and the cells overlap.
       Pinning the rows to their content contribution keeps the ratio and lets
       the overflow scroll instead. */
    grid-auto-rows: min-content;
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
