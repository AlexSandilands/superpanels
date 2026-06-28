<script lang="ts">
  import Icon from '../widgets/Icon.svelte';
  import { canvasView, type CanvasMode } from '$lib/stores/canvas-view.svelte';

  type Props = {
    mode: CanvasMode;
    onSetMode: (mode: CanvasMode) => void;
    onSnapWidth: () => void;
    onSnapHeight: () => void;
    onSnapCover: () => void;
    onResetTransform: () => void;
    onResetLayout: () => void;
    /** Cover / reset apply (a layer is selected, or the slideshow image loaded). */
    snapEnabled: boolean;
    /** Single-axis snaps apply (standard mode with a selected layer). */
    axisSnapEnabled: boolean;
  };
  let {
    mode,
    onSetMode,
    onSnapWidth,
    onSnapHeight,
    onSnapCover,
    onResetTransform,
    onResetLayout,
    snapEnabled,
    axisSnapEnabled,
  }: Props = $props();
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
  <button
    class="tool"
    class:tool--active={mode === 'images'}
    title="Move images (1)"
    onclick={() => onSetMode('images')}
  >
    <Icon name="move" />
  </button>
  <button
    class="tool"
    class:tool--active={mode === 'monitors'}
    title="Move monitors (2)"
    onclick={() => onSetMode('monitors')}
  >
    <Icon name="grid" />
  </button>
  <div style:height="1px" style:background="var(--line)" style:margin="4px 0"></div>
  <button
    class="tool"
    title="Snap image to monitor width (letterbox)"
    disabled={!axisSnapEnabled}
    onclick={onSnapWidth}
  >
    <Icon name="snap-w" />
  </button>
  <button
    class="tool"
    title="Snap image to monitor height (fill)"
    disabled={!axisSnapEnabled}
    onclick={onSnapHeight}
  >
    <Icon name="snap-h" />
  </button>
  <button class="tool" title="Snap image to cover" disabled={!snapEnabled} onclick={onSnapCover}>
    <Icon name="cover" />
  </button>
  <button
    class="tool"
    title="Reset image transform (R)"
    disabled={!snapEnabled}
    onclick={onResetTransform}
  >
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
  .tool:hover:not(:disabled) {
    background: var(--panel-2);
  }
  .tool:disabled {
    opacity: 0.3;
  }
  .tool--active {
    background: color-mix(in oklab, var(--accent) 16%, transparent);
    color: var(--accent);
  }
</style>
