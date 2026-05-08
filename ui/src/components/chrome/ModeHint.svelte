<script lang="ts">
  // Centre-bottom hint. Shifts up above the bezel/source docks when the window
  // narrows enough that they would overlap horizontally, and hides entirely
  // below a width where even the raised position is no longer useful.
  // Thresholds approximate the combined widths of BezelDock (~520px) and
  // SourceDock (~360px) plus the hint itself.
  const RAISE_BREAKPOINT = 2200;
  // Match SourceDock's STACK_BREAKPOINT — once that lifts the bottom-right dock
  // up onto the hint's row, the area gets too busy to be readable.
  const HIDE_BREAKPOINT = 1180;

  let innerWidth = $state(typeof window === 'undefined' ? 1920 : window.innerWidth);

  $effect(() => {
    const onResize = () => (innerWidth = window.innerWidth);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  const hidden = $derived(innerWidth < HIDE_BREAKPOINT);
  const raised = $derived(innerWidth < RAISE_BREAKPOINT);
</script>

{#if !hidden}
  <div
    class="absolute pointer-events-none"
    style:left="50%"
    style:bottom={raised ? '88px' : '86px'}
    style:transform={raised ? 'translate(-50%, -28px)' : 'translateX(-50%)'}
    style:background="color-mix(in oklab, var(--panel) 85%, transparent)"
    style:border="1px solid var(--line)"
    style:border-radius="16px"
    style:padding="4px 12px"
    style:display="flex"
    style:gap="10px"
    style:align-items="center"
    style:font-size="11px"
    style:color="var(--text-3)"
    style:backdrop-filter="blur(12px)"
    style:-webkit-backdrop-filter="blur(12px)"
    style:transition="bottom 140ms ease, transform 140ms ease"
    style:z-index="4"
  >
    <span><span style:color="var(--text-2)">Drag image</span> to pan</span>
    <span style:opacity="0.4">·</span>
    <span><span style:color="var(--text-2)">Drag a monitor</span> to rearrange</span>
    <span style:opacity="0.4">·</span>
    <span><span style:color="var(--text-2)">Scroll</span> to zoom</span>
    <span style:opacity="0.4">·</span>
    <span><span style:color="var(--text-2)">Alt</span> disables snap</span>
  </div>
{/if}
