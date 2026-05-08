<script lang="ts">
  type Props = { title: string; sub?: string };
  let { title, sub }: Props = $props();
</script>

<div style:margin-bottom="24px">
  <div style:font-size="18px" style:font-weight="600">{title}</div>
  {#if sub}
    <div style:font-size="12px" style:color="var(--text-3)" style:margin-top="4px">{sub}</div>
  {/if}
</div>
