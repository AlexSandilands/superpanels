<script lang="ts">
  import Backdrop from './Backdrop.svelte';

  type Props = {
    title: string;
    body: string;
    confirmLabel?: string;
    danger?: boolean;
    onCancel: () => void;
    onConfirm: () => void;
  };
  let {
    title,
    body,
    confirmLabel = 'Confirm',
    danger = false,
    onCancel,
    onConfirm,
  }: Props = $props();
</script>

<Backdrop onClose={onCancel}>
  <div class="panel dialog" role="dialog" aria-modal="true">
    <h3>{title}</h3>
    <p>{body}</p>
    <div class="actions">
      <button class="btn sm" onclick={onCancel}>Cancel</button>
      <button class="btn sm" class:danger onclick={onConfirm}>
        {confirmLabel}
      </button>
    </div>
  </div>
</Backdrop>

<style>
  .dialog {
    width: 380px;
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
  .danger {
    background: var(--danger);
    border-color: var(--danger);
    color: white;
    font-weight: 600;
  }
</style>
