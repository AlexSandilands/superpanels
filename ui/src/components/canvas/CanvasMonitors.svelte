<script lang="ts">
  // Presentational monitor outlines: border, name, resolution, and the "mm?"
  // physical-size-missing flag. Pointer-events:none — PreviewCanvas hit-tests
  // monitors against the same projected rects it passes in here.

  export type RenderedMonitor = {
    id: string;
    name: string;
    pxW: number;
    pxH: number;
    missing: boolean;
    x: number;
    y: number;
    w: number;
    h: number;
    isSel: boolean;
    isHover: boolean;
  };

  type Props = { monitors: RenderedMonitor[]; flashing: boolean };
  let { monitors, flashing }: Props = $props();
</script>

{#each monitors as m (m.id)}
  <div
    class="pointer-events-none absolute"
    style:left="{m.x}px"
    style:top="{m.y}px"
    style:width="{m.w}px"
    style:height="{m.h}px"
    style:border="1.5px solid {m.isSel
      ? 'var(--accent)'
      : m.isHover
        ? 'var(--text-2)'
        : 'var(--line-2)'}"
    style:border-radius="3px"
    style:transition="border-color 80ms, box-shadow 80ms"
    style:box-shadow={m.isSel
      ? '0 0 0 1px color-mix(in oklab, var(--accent) 30%, transparent), 0 0 24px color-mix(in oklab, var(--accent) 25%, transparent)'
      : m.isHover
        ? '0 0 12px oklch(1 0 0 / 0.15)'
        : 'none'}
    style:animation={flashing ? 'applyFlash 380ms ease-out' : 'none'}
  >
    <div
      class="pointer-events-none mono absolute font-semibold"
      style:top="6px"
      style:left="8px"
      style:font-size="10px"
      style:letter-spacing="0.04em"
      style:color={m.isSel ? 'var(--accent)' : 'var(--text-2)'}
      style:text-shadow="0 1px 2px oklch(0 0 0 / 0.6)"
    >
      {m.name}
    </div>
    <div
      class="pointer-events-none mono absolute"
      style:bottom="6px"
      style:right="8px"
      style:font-size="9px"
      style:color="var(--text-3)"
      style:text-shadow="0 1px 2px oklch(0 0 0 / 0.6)"
    >
      {m.pxW}×{m.pxH}
    </div>
    {#if m.missing}
      <div
        class="pointer-events-none absolute mono"
        style:top="6px"
        style:right="8px"
        style:font-size="9px"
        style:color="var(--warn)"
        style:text-shadow="0 1px 2px oklch(0 0 0 / 0.6)"
      >
        mm?
      </div>
    {/if}
  </div>
{/each}
