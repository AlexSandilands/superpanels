<script lang="ts">
  // 100mm reference grid behind the monitors. Hidden when the on-screen step
  // gets too small to be useful.

  type Props = { scale: number; ox: number; oy: number };
  let { scale, ox, oy }: Props = $props();

  const step = $derived(100 * scale);
  const startX = $derived(((ox % step) + step) % step);
  const startY = $derived(((oy % step) + step) % step);
</script>

{#if step >= 8}
  <svg class="pointer-events-none absolute inset-0" style:opacity="0.35" aria-hidden="true">
    <defs>
      <pattern
        id="sp-grid"
        x={startX}
        y={startY}
        width={step}
        height={step}
        patternUnits="userSpaceOnUse"
      >
        <path d="M {step} 0 L 0 0 0 {step}" fill="none" stroke="var(--line)" stroke-width="0.5"
        ></path>
      </pattern>
    </defs>
    <rect width="100%" height="100%" fill="url(#sp-grid)"></rect>
  </svg>
{/if}
