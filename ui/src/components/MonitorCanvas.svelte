<script lang="ts">
  import { onMount } from 'svelte';
  import type { Monitor } from '$lib/api';

  let { monitors = [] }: { monitors: Monitor[] } = $props();

  let canvasEl: HTMLDivElement | undefined = $state();
  let width = $state(800);
  let height = $state(450);

  onMount(() => {
    if (!canvasEl) return;
    const observer = new ResizeObserver(() => {
      if (!canvasEl) return;
      const rect = canvasEl.getBoundingClientRect();
      width = Math.max(1, Math.floor(rect.width));
      height = Math.max(1, Math.floor(rect.height));
    });
    observer.observe(canvasEl);
    return () => observer.disconnect();
  });

  // Convert each monitor's logical pixel position+size into canvas-space
  // rectangles that fit the available canvas with some padding.
  type Rect = { x: number; y: number; w: number; h: number; name: string; missing: boolean };

  const layout = $derived.by((): { rects: Rect[]; minX: number; minY: number; scale: number } => {
    if (monitors.length === 0) {
      return { rects: [], minX: 0, minY: 0, scale: 1 };
    }
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const m of monitors) {
      const x = m.position[0];
      const y = m.position[1];
      const w = m.resolution[0];
      const h = m.resolution[1];
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x + w > maxX) maxX = x + w;
      if (y + h > maxY) maxY = y + h;
    }
    const totalW = Math.max(1, maxX - minX);
    const totalH = Math.max(1, maxY - minY);
    const padding = 32;
    const scaleX = (width - padding * 2) / totalW;
    const scaleY = (height - padding * 2) / totalH;
    const scale = Math.max(0.05, Math.min(scaleX, scaleY));
    const rects: Rect[] = monitors.map((m) => ({
      x: padding + (m.position[0] - minX) * scale,
      y: padding + (m.position[1] - minY) * scale,
      w: m.resolution[0] * scale,
      h: m.resolution[1] * scale,
      name: m.name,
      missing: m.physical_size_mm === null,
    }));
    return { rects, minX, minY, scale };
  });
</script>

<div
  bind:this={canvasEl}
  class="relative h-full w-full overflow-hidden rounded border border-slate-800 bg-bezel"
>
  {#if monitors.length === 0}
    <div class="absolute inset-0 flex items-center justify-center text-sm text-slate-500">
      No monitors detected — try <code class="ml-1 rounded bg-slate-800 px-1"
        >superpanels detect --debug</code
      >
    </div>
  {:else}
    <svg
      class="absolute inset-0 h-full w-full"
      viewBox="0 0 {width} {height}"
      preserveAspectRatio="none"
      role="img"
      aria-label="Monitor layout preview"
    >
      {#each layout.rects as r, i (i)}
        <g>
          <rect
            x={r.x}
            y={r.y}
            width={r.w}
            height={r.h}
            fill={r.missing ? '#1f2937' : '#0f172a'}
            stroke={r.missing ? '#f59e0b' : '#60a5fa'}
            stroke-width="2"
            rx="4"
          ></rect>
          <text
            x={r.x + 8}
            y={r.y + 18}
            fill="#cbd5e1"
            font-size="12"
            font-family="ui-sans-serif, system-ui, sans-serif"
          >
            {r.name}
          </text>
          {#if r.missing}
            <text
              x={r.x + 8}
              y={r.y + r.h - 8}
              fill="#fbbf24"
              font-size="10"
              font-family="ui-sans-serif, system-ui, sans-serif"
            >
              physical size unknown
            </text>
          {/if}
        </g>
      {/each}
    </svg>
  {/if}
</div>
