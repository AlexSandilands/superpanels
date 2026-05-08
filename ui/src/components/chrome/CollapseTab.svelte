<script lang="ts">
  type Props = {
    side: 'left' | 'right';
    label: string;
    summary: string;
    onExpand: () => void;
    left?: number;
    right?: number;
    bottom?: number;
  };
  let { side, label, summary, onExpand, left, right, bottom = 14 }: Props = $props();

  const path = $derived(side === 'left' ? 'M3 2l3 3-3 3' : 'M5 2L2 5l3 3');
</script>

<button
  class="panel tab"
  style:left={left != null ? `${left}px` : 'auto'}
  style:right={right != null ? `${right}px` : 'auto'}
  style:bottom="{bottom}px"
  onclick={onExpand}
  title="Expand {label}"
>
  <span class="lbl">{label}</span>
  <span class="mono summary">{summary}</span>
  <svg width="9" height="10" viewBox="0 0 8 10" aria-hidden="true">
    <path
      d={path}
      stroke="currentColor"
      stroke-width="1.4"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
</button>

<style>
  .tab {
    position: absolute;
    height: 32px;
    padding: 0 10px 0 12px;
    border-radius: 16px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    z-index: 5;
    color: var(--text-3);
    font: inherit;
  }
  .lbl {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .summary {
    font-size: 11px;
    color: var(--text-2);
  }
</style>
