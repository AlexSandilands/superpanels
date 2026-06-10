<script lang="ts">
  import type { ProfileKind } from '$lib/types/profile-helpers';

  type Props = {
    existingNames: string[];
    defaultName: string;
    hasSource: boolean;
    onCancel: () => void;
    onConfirm: (name: string, description: string | null, kind: ProfileKind) => void;
  };
  let { existingNames, defaultName, hasSource, onCancel, onConfirm }: Props = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(defaultName);
  let description = $state('');
  // svelte-ignore state_referenced_locally
  let kind = $state<ProfileKind>(hasSource ? 'single' : 'slideshow');

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
    onConfirm(name.trim(), description.trim() || null, kind);
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
      <div class="kind-row" role="radiogroup" aria-label="Profile type">
        <button
          type="button"
          class:kind-active={kind === 'single'}
          disabled={!hasSource}
          title={hasSource ? 'One image spanning the desktop' : 'Pick a source image first'}
          onclick={() => (kind = 'single')}
        >
          Single image
        </button>
        <button
          type="button"
          class:kind-active={kind === 'slideshow'}
          title="Rotate through a set of images and folders"
          onclick={() => (kind = 'slideshow')}
        >
          Slideshow
        </button>
      </div>
      {#if kind === 'slideshow'}
        <p class="hint">
          {hasSource
            ? 'Starts with the current image — add more from the library after saving.'
            : 'You’ll pick images and folders from the library after saving.'}
        </p>
      {/if}
      <label
        >Name
        <input type="text" bind:value={name} use:focusOnMount />
        {#if nameError}<span class="err">{nameError}</span>{/if}
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
  .kind-row {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  .kind-row button {
    flex: 1;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 500;
    font-family: inherit;
  }
  .kind-row button:hover:not(:disabled) {
    background: var(--panel-2);
  }
  .kind-row button:disabled {
    opacity: 0.4;
  }
  .kind-active {
    background: var(--accent) !important;
    color: oklch(0.16 0.01 250) !important;
  }
  .hint {
    margin: 0;
    font-size: 11px;
    color: var(--text-3);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
