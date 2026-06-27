<script lang="ts">
  // Presentational composite-layer rendering. PreviewCanvas owns hit-testing and
  // pointer routing (everything here is pointer-events:none); this just paints
  // each layer's image plus, on the hovered layer, the resize handle and the ✕
  // remove affordance whose hit regions PreviewCanvas detects.

  export type RenderedLayer = {
    id: string;
    url: string | null;
    x: number;
    y: number;
    w: number;
    h: number;
    dimmed: boolean;
    hovered: boolean;
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
      style:box-shadow={l.hovered
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
      style:opacity={l.hovered ? '0.95' : '0.55'}
    ></div>

    {#if l.hovered}
      <!-- remove ✕ (top-right) -->
      <div
        class="pointer-events-none absolute flex items-center justify-center rounded-full"
        style:left="{l.x + l.w - 22}px"
        style:top="{l.y + 10}px"
        style:width="20px"
        style:height="20px"
        style:background="var(--danger)"
        style:color="#fff"
        style:font-size="13px"
        style:font-weight="700"
        style:line-height="1"
        style:box-shadow="0 1px 4px oklch(0 0 0 / 0.5)"
      >
        ×
      </div>
    {/if}
  {/if}
{/each}
