<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = { onClose: () => void; children: Snippet };
  let { onClose, children }: Props = $props();

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
