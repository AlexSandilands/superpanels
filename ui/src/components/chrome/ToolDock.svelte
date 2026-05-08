<script lang="ts">
  import Icon from '../Icon.svelte';
  import { canvasView } from '$lib/stores/canvasView.svelte';

  type Props = {
    onResetTransform: () => void;
    onSnapCover: () => void;
    onResetLayout: () => void;
  };
  let { onResetTransform, onSnapCover, onResetLayout }: Props = $props();
</script>

<div
  class="panel absolute flex flex-col"
  style:left="14px"
  style:top="56px"
  style:padding="6px"
  style:gap="4px"
  style:width="44px"
  style:z-index="5"
>
  <button class="tool tool--active" title="Move (auto)">
    <Icon name="move" />
  </button>
  <button class="tool" title="Snap image to cover" onclick={onSnapCover}>
    <Icon name="cover" />
  </button>
  <button class="tool" title="Reset image transform (R)" onclick={onResetTransform}>
    <Icon name="reset" />
  </button>
  <button class="tool" title="Reset monitor layout" onclick={onResetLayout}>
    <Icon name="layout" />
  </button>
  <div style:height="1px" style:background="var(--line)" style:margin="4px 0"></div>
  <button
    class="tool"
    class:tool--active={canvasView.dim}
    title="Off-monitor dim (D)"
    onclick={() => canvasView.toggleDim()}
  >
    <Icon name="dim" />
  </button>
  <div style:height="1px" style:background="var(--line)" style:margin="4px 0"></div>
  <button
    class="btn ghost icon sm"
    title="Zoom in"
    onclick={() => canvasView.setZoom(canvasView.zoom + 0.1)}
  >
    <Icon name="plus" size={12} />
  </button>
  <div class="mono" style:text-align="center" style:font-size="9px" style:color="var(--text-3)">
    {Math.round(canvasView.zoom * 100)}%
  </div>
  <button
    class="btn ghost icon sm"
    title="Zoom out"
    onclick={() => canvasView.setZoom(canvasView.zoom - 0.1)}
  >
    <Icon name="minus" size={12} />
  </button>
  <button
    class="btn ghost icon sm"
    title="Fit"
    onclick={() => {
      canvasView.setZoom(1);
      canvasView.resetPan();
    }}
  >
    <Icon name="fit" size={12} />
  </button>
</div>

<style>
  .tool {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--text-2);
    width: 32px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    transition: background 80ms;
  }
  .tool:hover {
    background: var(--panel-2);
  }
  .tool--active {
    background: color-mix(in oklab, var(--accent) 16%, transparent);
    color: var(--accent);
  }
</style>
