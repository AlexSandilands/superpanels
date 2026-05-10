<script lang="ts">
  import type { TopologyRect } from '$lib/profile-topology';

  type Props = {
    rects: TopologyRect[];
    width?: number;
    height?: number;
    color?: string;
  };
  let { rects, width = 92, height = 36, color = 'var(--text-2)' }: Props = $props();

  const view = $derived.by(() => {
    if (rects.length === 0) return null;
    const xs = rects.flatMap((r) => [r.x, r.x + r.w]);
    const ys = rects.flatMap((r) => [r.y, r.y + r.h]);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const bbW = Math.max(1, maxX - minX);
    const bbH = Math.max(1, maxY - minY);
    const s = Math.min(width / bbW, height / bbH) * 0.92;
    return {
      s,
      offX: (width - bbW * s) / 2 - minX * s,
      offY: (height - bbH * s) / 2 - minY * s,
    };
  });
</script>

{#if view}
  <svg {width} {height} viewBox="0 0 {width} {height}" style:display="block">
    {#each rects as r, i (i)}
      <rect
        x={r.x * view.s + view.offX}
        y={r.y * view.s + view.offY}
        width={r.w * view.s}
        height={r.h * view.s}
        rx="1"
        fill="none"
        stroke={color}
        stroke-width="1.1"
      ></rect>
    {/each}
  </svg>
{:else}
  <div class="placeholder" style:width="{width}px" style:height="{height}px"></div>
{/if}

<style>
  .placeholder {
    border: 1px dashed var(--line);
    border-radius: 3px;
  }
</style>
