<script lang="ts">
  // Presentational composite-layer rendering. PreviewCanvas owns hit-testing and
  // pointer routing (everything here is pointer-events:none); this paints, in
  // three passes: (1) the layer images, dimmed in dim/backdrop mode; (2) when
  // the off-monitor dim is on, a bright per-monitor window compositing the
  // layers that overlap it (mirrors CanvasSpanImage + the apply-time composite,
  // so monitors stay bright while everything off-screen reads dim); (3) the
  // crisp affordances — selection ring, resize handle, and the top-right button
  // cluster (snap-width, snap-height, remove) — on top. Button centres come from
  // the shared layer-buttons geometry that hit-test.ts also reads.

  import Icon from '../widgets/Icon.svelte';
  import {
    LAYER_BTN_SIZE,
    LAYER_BTN_TOP,
    REMOVE_CX,
    SNAP_W_CX,
    SNAP_H_CX,
  } from '$lib/canvas/layer-buttons';

  const BTN_HALF = LAYER_BTN_SIZE / 2;

  type Rect = { x: number; y: number; w: number; h: number };

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

  type Props = {
    layers: RenderedLayer[];
    dragging: boolean;
    dim: boolean;
    monitorRects: Rect[];
  };
  let { layers, dragging, dim, monitorRects }: Props = $props();
</script>

<!-- Pass 1: layer images (dimmed in dim / monitor-edit mode) -->
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
    ></div>
  {/if}
{/each}

<!-- Pass 2: bright per-monitor composite windows (off-monitor dim) -->
{#if dim}
  {#each monitorRects as m, mi (mi)}
    <div
      class="pointer-events-none absolute"
      style:left="{m.x}px"
      style:top="{m.y}px"
      style:width="{m.w}px"
      style:height="{m.h}px"
      style:overflow="hidden"
    >
      {#each layers as l (l.id)}
        {#if l.url}
          <div
            class="absolute"
            style:left="{l.x - m.x}px"
            style:top="{l.y - m.y}px"
            style:width="{l.w}px"
            style:height="{l.h}px"
            style:background-image="url({l.url})"
            style:background-size="100% 100%"
            style:background-repeat="no-repeat"
          ></div>
        {/if}
      {/each}
    </div>
  {/each}
{/if}

<!-- Pass 3: affordances (ring, resize handle, buttons) -->
{#each layers as l (l.id)}
  {#if l.url}
    <div
      class="pointer-events-none absolute"
      style:left="{l.x}px"
      style:top="{l.y}px"
      style:width="{l.w}px"
      style:height="{l.h}px"
      style:box-shadow={l.hovered || l.selected
        ? '0 0 0 1.5px var(--accent), 0 0 18px color-mix(in oklab, var(--accent) 25%, transparent)'
        : '0 0 0 1px oklch(1 0 0 / 0.06)'}
    ></div>

    <!-- resize handles (bottom-right + top-left) -->
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
    <div
      class="pointer-events-none absolute rounded-sm"
      style:left="{l.x - 6}px"
      style:top="{l.y - 6}px"
      style:width="12px"
      style:height="12px"
      style:background="var(--accent)"
      style:border="2px solid var(--bg)"
      style:opacity={l.hovered || l.selected ? '0.95' : '0.55'}
    ></div>

    {#if l.hovered && l.showButtons}
      {@const btnTop = l.y + LAYER_BTN_TOP - BTN_HALF}
      <!-- snap to monitor height (fit vertically) -->
      <div
        class="layer-btn neutral"
        style:width="{LAYER_BTN_SIZE}px"
        style:height="{LAYER_BTN_SIZE}px"
        style:left="{l.x + l.w - SNAP_H_CX - BTN_HALF}px"
        style:top="{btnTop}px"
      >
        <Icon name="snap-h" size={12} />
      </div>
      <!-- snap to monitor width (fit horizontally) -->
      <div
        class="layer-btn neutral"
        style:width="{LAYER_BTN_SIZE}px"
        style:height="{LAYER_BTN_SIZE}px"
        style:left="{l.x + l.w - SNAP_W_CX - BTN_HALF}px"
        style:top="{btnTop}px"
      >
        <Icon name="snap-w" size={12} />
      </div>
      <!-- remove -->
      <div
        class="layer-btn accent"
        style:width="{LAYER_BTN_SIZE}px"
        style:height="{LAYER_BTN_SIZE}px"
        style:left="{l.x + l.w - REMOVE_CX - BTN_HALF}px"
        style:top="{btnTop}px"
      >
        <Icon name="trash" size={12} />
      </div>
    {/if}
  {/if}
{/each}

<style>
  .layer-btn {
    position: absolute;
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
