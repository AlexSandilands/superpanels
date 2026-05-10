<script lang="ts">
  import type { ProfileColour } from '$lib/types/ProfileColour';

  type Props = {
    existingNames: string[];
    defaultName: string;
    onCancel: () => void;
    onConfirm: (name: string, colour: ProfileColour, description: string | null) => void;
  };
  let { existingNames, defaultName, onCancel, onConfirm }: Props = $props();

  const SWATCHES: ProfileColour[] = [
    'slate',
    'stone',
    'red',
    'orange',
    'amber',
    'yellow',
    'lime',
    'emerald',
    'teal',
    'sky',
    'indigo',
    'violet',
  ];

  // svelte-ignore state_referenced_locally
  let name = $state(defaultName);
  let colour = $state<ProfileColour>('slate');
  let description = $state('');

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
  }

  let nameError = $derived.by(() => {
    if (!name.trim()) return 'name is required';
    if (existingNames.includes(name.trim())) return 'name is already taken';
    return null;
  });

  function submit(e: Event) {
    e.preventDefault();
    if (nameError) return;
    onConfirm(name.trim(), colour, description.trim() || null);
  }
</script>

<div
  class="backdrop"
  onclick={onCancel}
  onkeydown={(e) => e.key === 'Escape' && onCancel()}
  role="presentation"
>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    tabindex={-1}
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    <h3>Save as new profile</h3>
    <form onsubmit={submit}>
      <label
        >Name
        <input type="text" bind:value={name} use:focusOnMount />
        {#if nameError}<span class="err">{nameError}</span>{/if}
      </label>
      <label
        >Colour
        <div class="swatches">
          {#each SWATCHES as c (c)}
            <button
              type="button"
              class="swatch"
              class:selected={colour === c}
              data-colour={c}
              onclick={() => (colour = c)}
              aria-label={c}
              title={c}
            ></button>
          {/each}
        </div>
      </label>
      <label
        >Description (optional)
        <textarea bind:value={description} rows="2"></textarea>
      </label>
      <div class="actions">
        <button type="button" class="btn sm" onclick={onCancel}>Cancel</button>
        <button type="submit" class="btn sm primary" disabled={!!nameError}>Save</button>
      </div>
    </form>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: oklch(0 0 0 / 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 18px;
    width: 360px;
  }
  h3 {
    margin: 0 0 14px;
    font-size: 14px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
  }
  input,
  textarea {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 6px 8px;
    font-family: inherit;
  }
  .err {
    color: var(--err);
    font-size: 11px;
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(12, 18px);
    gap: 4px;
  }
  .swatch {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid var(--line);
    cursor: pointer;
    padding: 0;
  }
  .swatch[data-colour='slate'] {
    background: oklch(0.65 0.04 250);
  }
  .swatch[data-colour='stone'] {
    background: oklch(0.7 0.02 80);
  }
  .swatch[data-colour='red'] {
    background: oklch(0.62 0.2 25);
  }
  .swatch[data-colour='orange'] {
    background: oklch(0.7 0.18 50);
  }
  .swatch[data-colour='amber'] {
    background: oklch(0.78 0.17 80);
  }
  .swatch[data-colour='yellow'] {
    background: oklch(0.85 0.18 100);
  }
  .swatch[data-colour='lime'] {
    background: oklch(0.78 0.2 130);
  }
  .swatch[data-colour='emerald'] {
    background: oklch(0.7 0.18 160);
  }
  .swatch[data-colour='teal'] {
    background: oklch(0.7 0.13 200);
  }
  .swatch[data-colour='sky'] {
    background: oklch(0.7 0.15 235);
  }
  .swatch[data-colour='indigo'] {
    background: oklch(0.55 0.2 270);
  }
  .swatch[data-colour='violet'] {
    background: oklch(0.6 0.22 300);
  }
  .swatch.selected {
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
