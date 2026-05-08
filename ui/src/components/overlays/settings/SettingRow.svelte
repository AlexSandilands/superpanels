<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = { label: string; sub?: string; children: Snippet };
  let { label, sub, children }: Props = $props();
</script>

<div class="row">
  <div style:flex="1" style:min-width="0">
    <div style:font-size="13px" style:font-weight="500">{label}</div>
    {#if sub}
      <div style:font-size="11px" style:color="var(--text-3)" style:margin-top="2px">{sub}</div>
    {/if}
  </div>
  <div>{@render children()}</div>
</div>

<style>
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    gap: 16px;
  }
</style>
