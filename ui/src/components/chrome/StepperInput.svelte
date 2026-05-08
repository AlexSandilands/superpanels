<script lang="ts">
  type Props = {
    label?: string;
    value: number;
    onChange: (v: number) => void;
    unit?: string;
    step?: number;
    bigStep?: number;
    min?: number;
    max?: number;
    decimals?: number;
    width?: number;
  };

  let {
    label,
    value,
    onChange,
    unit = '',
    step = 0.5,
    bigStep = 5,
    min = 0,
    max = Number.POSITIVE_INFINITY,
    decimals = 1,
    width = 44,
  }: Props = $props();

  let hover = $state(false);

  function clamp(n: number): number {
    if (Number.isNaN(n)) return min;
    return Math.max(min, Math.min(max, n));
  }

  function bump(delta: number, big: boolean) {
    const inc = (big ? bigStep : step) * delta;
    onChange(clamp(parseFloat((value + inc).toFixed(Math.max(decimals, 4)))));
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    bump(e.deltaY < 0 ? 1 : -1, e.shiftKey);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      bump(1, e.shiftKey);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      bump(-1, e.shiftKey);
    }
  }

  function onInput(e: Event) {
    const v = parseFloat((e.currentTarget as HTMLInputElement).value);
    if (!Number.isNaN(v)) onChange(clamp(v));
  }
</script>

<div
  class="stepper"
  class:hover
  onmouseenter={() => (hover = true)}
  onmouseleave={() => (hover = false)}
  onwheel={onWheel}
  title="Scroll, ↑/↓, or use the steppers (Shift = ×{Math.round(bigStep / step)})"
  role="group"
>
  {#if label}
    <span class="lbl mono">{label}</span>
  {/if}
  <input
    type="text"
    inputmode="decimal"
    class="num mono"
    style:width="{width}px"
    value={value.toFixed(decimals)}
    oninput={onInput}
    onkeydown={onKey}
  />
  {#if unit}
    <span class="unit mono">{unit}</span>
  {/if}
  <div class="steppers">
    <button class="step" tabindex={-1} onclick={(e) => bump(1, e.shiftKey)} aria-label="Increase">
      <svg width="7" height="5" viewBox="0 0 7 5" aria-hidden="true">
        <path
          d="M1 4l2.5-3 2.5 3"
          stroke="currentColor"
          stroke-width="1.2"
          fill="none"
          stroke-linecap="round"
          stroke-linejoin="round"
        ></path>
      </svg>
    </button>
    <div class="step-divider"></div>
    <button class="step" tabindex={-1} onclick={(e) => bump(-1, e.shiftKey)} aria-label="Decrease">
      <svg width="7" height="5" viewBox="0 0 7 5" aria-hidden="true">
        <path
          d="M1 1l2.5 3 2.5-3"
          stroke="currentColor"
          stroke-width="1.2"
          fill="none"
          stroke-linecap="round"
          stroke-linejoin="round"
        ></path>
      </svg>
    </button>
  </div>
  <span class="wheel-hint" title="Scroll wheel adjusts" aria-hidden="true">
    <svg width="10" height="14" viewBox="0 0 10 14">
      <rect
        x="1"
        y="1"
        width="8"
        height="12"
        rx="4"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
      ></rect>
      <rect x="4.3" y="3.5" width="1.4" height="3" rx="0.7" fill="currentColor"></rect>
      <path
        d="M5 10v1.6M3.6 11l1.4 1.4 1.4-1.4"
        stroke="currentColor"
        stroke-width="0.8"
        fill="none"
        stroke-linecap="round"
        stroke-linejoin="round"
        opacity="0.7"
      ></path>
    </svg>
  </span>
</div>

<style>
  .stepper {
    display: inline-flex;
    align-items: stretch;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--bg-2);
    overflow: hidden;
    transition: border-color 80ms;
  }
  .stepper.hover {
    border-color: var(--line-2);
  }
  .lbl {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    padding: 0 4px;
    font-size: 10px;
    color: var(--text-3);
    border-right: 1px solid var(--line);
    background: color-mix(in oklab, var(--bg) 60%, transparent);
  }
  .num {
    height: 100%;
    padding: 0 6px;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    text-align: right;
    font-size: 12px;
  }
  .unit {
    display: inline-flex;
    align-items: center;
    padding-right: 4px;
    font-size: 10px;
    color: var(--text-3);
  }
  .steppers {
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--line);
    opacity: 0.55;
    transition: opacity 100ms;
  }
  .stepper.hover .steppers {
    opacity: 1;
  }
  .step {
    appearance: none;
    border: none;
    background: transparent;
    width: 16px;
    flex: 1;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-3);
    transition:
      background 60ms,
      color 60ms;
  }
  .step:hover {
    background: var(--line);
    color: var(--text);
  }
  .step-divider {
    height: 1px;
    background: var(--line);
  }
  .wheel-hint {
    display: none;
    align-items: center;
    justify-content: center;
    width: 18px;
    color: var(--accent);
    border-left: 1px solid var(--line);
    background: color-mix(in oklab, var(--accent) 10%, transparent);
  }
  .stepper.hover .wheel-hint {
    display: inline-flex;
  }
</style>
