<script lang="ts">
  // CAD-style dashed dimension lines + mm labels between adjacent monitors.
  // Drawn over the absolute-positioned monitor rectangles in screen space.

  type Line = { x1: number; y1: number; x2: number; y2: number; label: string };

  let { lines }: { lines: Line[] } = $props();
</script>

<svg class="pointer-events-none absolute inset-0" style:overflow="visible" aria-hidden="true">
  {#each lines as l, i (i)}
    {@const cx = (l.x1 + l.x2) / 2}
    {@const cy = (l.y1 + l.y2) / 2}
    {@const isVertical = Math.abs(l.x2 - l.x1) < Math.abs(l.y2 - l.y1)}
    <g stroke="var(--accent)" stroke-opacity="0.85" fill="var(--accent)">
      <line x1={l.x1} y1={l.y1} x2={l.x2} y2={l.y2} stroke-width="1" stroke-dasharray="3 3"></line>
      {#if isVertical}
        <line x1={l.x1 - 5} y1={l.y1} x2={l.x1 + 5} y2={l.y1} stroke-width="1"></line>
        <line x1={l.x2 - 5} y1={l.y2} x2={l.x2 + 5} y2={l.y2} stroke-width="1"></line>
      {:else}
        <line x1={l.x1} y1={l.y1 - 5} x2={l.x1} y2={l.y1 + 5} stroke-width="1"></line>
        <line x1={l.x2} y1={l.y2 - 5} x2={l.x2} y2={l.y2 + 5} stroke-width="1"></line>
      {/if}
      <rect
        x={cx - 24}
        y={cy - 9}
        width="48"
        height="18"
        rx="3"
        fill="var(--bg)"
        stroke="var(--accent)"
        stroke-opacity="0.4"
        stroke-width="0.5"
      ></rect>
      <text
        x={cx}
        y={cy + 4}
        text-anchor="middle"
        font-family="var(--mono)"
        font-size="10"
        fill="var(--accent)"
        stroke="none"
        font-weight="600"
      >
        {l.label}
      </text>
    </g>
  {/each}
</svg>
