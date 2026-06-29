<script lang="ts">
  import type { Snippet } from 'svelte';

  // `passthrough` hides the overlay and lets pointer/drag events fall through to
  // whatever is behind it — used while dragging a library image onto the canvas,
  // where the modal must stay mounted (so the native drag survives) but get out
  // of the way visually and for hit-testing.
  type Props = { onClose: () => void; children: Snippet; passthrough?: boolean };
  let { onClose, children, passthrough = false }: Props = $props();

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose();
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-30 flex items-center justify-center"
  class:passthrough
  style:background="oklch(0 0 0 / 0.6)"
  style:animation="fadeIn 120ms ease"
  onclick={onClose}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div onclick={(e) => e.stopPropagation()}>
    {@render children()}
  </div>
</div>

<style>
  .passthrough {
    opacity: 0;
    pointer-events: none;
  }
</style>
