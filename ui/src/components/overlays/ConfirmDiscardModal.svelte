<script lang="ts">
  // Modal shown before any action that would silently drop unsaved canvas
  // edits — user-initiated profile switch and window close (§4e.11.5,
  // §9.1.2). Schedule-driven switches surface a toast instead, so the
  // schedule preemption rule (§9.3.3) stays intact.
  import ConfirmDialog from '../widgets/ConfirmDialog.svelte';

  type Props = {
    activeName: string | null;
    actionLabel: string;
    onCancel: () => void;
    onConfirm: () => void;
  };
  let { activeName, actionLabel, onCancel, onConfirm }: Props = $props();

  const title = $derived(
    activeName ? `Discard unsaved changes to '${activeName}'?` : 'Discard unsaved canvas changes?',
  );
  const body = $derived(
    `${actionLabel} will drop the canvas edits. Save first if you want to keep them.`,
  );
</script>

<ConfirmDialog {title} {body} confirmLabel="Discard" danger {onCancel} {onConfirm} />
