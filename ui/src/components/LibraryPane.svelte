<script lang="ts">
  import LibraryGrid from './LibraryGrid.svelte';

  type Props = {
    onApplyAsSpan: (path: string) => void;
  };
  let { onApplyAsSpan }: Props = $props();
</script>

<LibraryGrid {onApplyAsSpan} />
