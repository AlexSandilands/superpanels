<script lang="ts">
  type Props = {
    side: 'left' | 'right';
    onClick: () => void;
    title?: string;
  };
  let { side, onClick, title = 'Collapse' }: Props = $props();

  const path = $derived(side === 'left' ? 'M5 2L2 5l3 3' : 'M3 2l3 3-3 3');
</script>

<button
  class="chev"
  class:left={side === 'left'}
  class:right={side === 'right'}
  onclick={onClick}
  {title}
  aria-label={title}
>
  <svg width="8" height="10" viewBox="0 0 8 10" aria-hidden="true">
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
  .chev {
    position: absolute;
    top: 50%;
    width: 20px;
    height: 28px;
    border-radius: 4px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    color: var(--text-3);
    transform: translateY(-50%);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition:
      background 80ms,
      color 80ms;
  }
  .chev:hover {
    background: var(--line);
    color: var(--text);
  }
  .chev.left {
    right: -10px;
  }
  .chev.right {
    left: -10px;
  }
</style>
