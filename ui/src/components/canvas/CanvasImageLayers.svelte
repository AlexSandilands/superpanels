<script lang="ts">
  // Presentational composite-layer rendering. PreviewCanvas owns hit-testing and
  // pointer routing (everything here is pointer-events:none); this just paints
  // each layer's image plus, on the hovered layer, the resize handle and the
  // top-right button cluster (snap-width, snap-height, remove) whose hit regions
  // PreviewCanvas detects. Button centres mirror hit-test.ts: remove 12 px in
  // from the right edge, snap-width 38 px, snap-height 64 px, all 20 px down.

  import Icon from '../widgets/Icon.svelte';

  export type RenderedLayer = {
    id: string;
    url: string | null;
    x: number;
    y: number;
    w: number;
    h: number;
    dimmed: boolean;
    selected: boolean;
    hovered: boolean;
    showButtons: boolean;
  };

  type Props = { layers: RenderedLayer[]; dragging: boolean };
  let { layers, dragging }: Props = $props();
</script>

{#each layers as l (l.id)}
  {#if l.url}
    <div
      class="pointer-events-none absolute"
      style:left="{l.x}px"
      style:top="{l.y}px"
      style:width="{l.w}px"
      style:height="{l.h}px"
      style:background-image="url({l.url})"
      style:background-size="100% 100%"
      style:opacity={l.dimmed ? '0.18' : '1'}
      style:transition={dragging ? 'none' : 'opacity 200ms ease'}
      style:box-shadow={l.hovered || l.selected
        ? '0 0 0 1.5px var(--accent), 0 0 18px color-mix(in oklab, var(--accent) 25%, transparent)'
        : '0 0 0 1px oklch(1 0 0 / 0.06)'}
    ></div>

    <!-- resize handle (bottom-right) -->
    <div
      class="pointer-events-none absolute rounded-sm"
      style:left="{l.x + l.w - 6}px"
      style:top="{l.y + l.h - 6}px"
      style:width="12px"
      style:height="12px"
      style:background="var(--accent)"
      style:border="2px solid var(--bg)"
      style:opacity={l.hovered || l.selected ? '0.95' : '0.55'}
    ></div>

    {#if l.hovered && l.showButtons}
      <!-- snap to monitor height (fit vertically) -->
      <div class="layer-btn neutral" style:left="{l.x + l.w - 74}px" style:top="{l.y + 10}px">
        <Icon name="snap-h" size={12} />
      </div>
      <!-- snap to monitor width (fit horizontally) -->
      <div class="layer-btn neutral" style:left="{l.x + l.w - 48}px" style:top="{l.y + 10}px">
        <Icon name="snap-w" size={12} />
      </div>
      <!-- remove -->
      <div class="layer-btn accent" style:left="{l.x + l.w - 22}px" style:top="{l.y + 10}px">
        <Icon name="trash" size={12} />
      </div>
    {/if}
  {/if}
{/each}

<style>
  .layer-btn {
    position: absolute;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 5px;
    pointer-events: none;
    box-shadow: 0 1px 4px oklch(0 0 0 / 0.45);
    backdrop-filter: blur(2px);
  }
  .layer-btn.neutral {
    background: oklch(0 0 0 / 0.45);
    color: oklch(1 0 0 / 0.85);
    border: 1px solid oklch(1 0 0 / 0.12);
  }
  .layer-btn.accent {
    background: color-mix(in oklab, var(--accent) 80%, oklch(0 0 0 / 0.4));
    color: oklch(0.16 0.01 250);
    border: 1px solid color-mix(in oklab, var(--accent) 60%, transparent);
  }
</style>
