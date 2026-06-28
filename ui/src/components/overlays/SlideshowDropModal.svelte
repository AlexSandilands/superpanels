<script lang="ts">
  // Shown when an image is added to the canvas while a slideshow profile is
  // being edited: dropping it would otherwise silently discard the slideshow's
  // source/timer/overrides. Offers an explicit choice instead of converting.
  import Backdrop from '../widgets/Backdrop.svelte';

  type Props = {
    fileName: string;
    onAddToSet: () => void;
    onConvert: () => void;
    onCancel: () => void;
  };
  let { fileName, onAddToSet, onConvert, onCancel }: Props = $props();
</script>

<Backdrop onClose={onCancel}>
  <div class="panel dialog" role="dialog" aria-modal="true">
    <h3>This profile is a slideshow</h3>
    <p>
      Add <span class="mono">{fileName}</span> to the slideshow's image set, or convert this profile to
      a standard canvas? Converting discards the slideshow's folders, timer, and per-image tweaks.
    </p>
    <div class="actions">
      <button class="btn sm" onclick={onCancel}>Cancel</button>
      <button class="btn sm" onclick={onConvert}>Convert to standard</button>
      <button class="btn sm primary" onclick={onAddToSet}>Add to slideshow set</button>
    </div>
  </div>
</Backdrop>

<style>
  .dialog {
    width: 420px;
    padding: 20px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 14px;
    font-weight: 600;
  }
  p {
    margin: 0 0 18px;
    font-size: 12px;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: oklch(0.16 0.01 250);
    font-weight: 600;
  }
  .btn.primary:hover:not(:disabled) {
    background: color-mix(in oklab, var(--accent) 88%, white);
    border-color: color-mix(in oklab, var(--accent) 88%, white);
  }
</style>
