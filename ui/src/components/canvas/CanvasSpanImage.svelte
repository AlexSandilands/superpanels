<script lang="ts">
  // Presentational single-image (span) rendering: the floating image, the
  // dim-mode per-monitor crop preview, and the resize handle. Pointer-events:
  // none — PreviewCanvas hit-tests the image and resize handle itself.

  type Rect = { x: number; y: number; w: number; h: number };

  type Props = {
    imageUrl: string;
    rect: Rect;
    dim: boolean;
    dragging: boolean;
    monitorRects: Rect[];
  };
  let { imageUrl, rect, dim, dragging, monitorRects }: Props = $props();
</script>

<div
  class="pointer-events-none absolute"
  style:left="{rect.x}px"
  style:top="{rect.y}px"
  style:width="{rect.w}px"
  style:height="{rect.h}px"
  style:background-image="url({imageUrl})"
  style:background-size="100% 100%"
  style:opacity={dim ? '0.18' : '1'}
  style:transition={dragging ? 'none' : 'opacity 200ms ease'}
  style:box-shadow={dim ? 'none' : '0 0 0 1px oklch(1 0 0 / 0.06)'}
></div>

{#if dim}
  {#each monitorRects as a, i (i)}
    <div
      class="pointer-events-none absolute"
      style:left="{a.x}px"
      style:top="{a.y}px"
      style:width="{a.w}px"
      style:height="{a.h}px"
      style:background-image="url({imageUrl})"
      style:background-size="{rect.w}px {rect.h}px"
      style:background-position="{rect.x - a.x}px {rect.y - a.y}px"
      style:background-repeat="no-repeat"
    ></div>
  {/each}
{/if}

<div
  class="pointer-events-none absolute rounded-sm"
  style:left="{rect.x + rect.w - 6}px"
  style:top="{rect.y + rect.h - 6}px"
  style:width="12px"
  style:height="12px"
  style:background="var(--accent)"
  style:border="2px solid var(--bg)"
  style:opacity="0.85"
></div>
<div
  class="pointer-events-none absolute rounded-sm"
  style:left="{rect.x - 6}px"
  style:top="{rect.y - 6}px"
  style:width="12px"
  style:height="12px"
  style:background="var(--accent)"
  style:border="2px solid var(--bg)"
  style:opacity="0.85"
></div>
