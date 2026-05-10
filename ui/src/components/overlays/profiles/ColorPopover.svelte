<script lang="ts">
  import type { ProfileColour } from '$lib/types/ProfileColour';
  import { PROFILE_COLOURS, profileColourCss } from '$lib/profile-colours';

  type Props = {
    value: ProfileColour;
    onPick: (c: ProfileColour) => void;
    onClose: () => void;
  };
  let { value, onPick, onClose }: Props = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dismiss-shield" onclick={onClose}></div>
<div class="popover panel">
  {#each PROFILE_COLOURS as c (c)}
    <button
      type="button"
      class="swatch"
      class:selected={value === c}
      style:background={profileColourCss(c)}
      onclick={() => onPick(c)}
      aria-label={c}
      title={c}
    ></button>
  {/each}
</div>

<style>
  .dismiss-shield {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: transparent;
  }
  .popover {
    position: absolute;
    top: 30px;
    left: 0;
    padding: 8px;
    display: grid;
    grid-template-columns: repeat(6, 22px);
    gap: 6px;
    z-index: 51;
    width: auto;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 3px;
    padding: 0;
    border: 1px solid var(--line);
    cursor: default;
  }
  .swatch.selected {
    border: 2px solid var(--accent);
    outline: 1px solid var(--bg);
  }
</style>
