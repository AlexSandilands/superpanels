<script lang="ts">
  type Props = { value: boolean; onChange: (v: boolean) => void };
  let { value, onChange }: Props = $props();
</script>

<button
  class="t"
  class:t-on={value}
  onclick={() => onChange(!value)}
  aria-pressed={value}
  aria-label="Toggle"
>
  <span class="knob" style:left={value ? '18px' : '2px'}></span>
</button>

<style>
  .t {
    width: 36px;
    height: 20px;
    border-radius: 10px;
    background: var(--line-2);
    border: none;
    position: relative;
    padding: 0;
    transition: background 120ms;
  }
  .t-on {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: white;
    transition: left 120ms;
  }
</style>
