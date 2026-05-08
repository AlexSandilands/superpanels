<script lang="ts">
  import type { FitMode } from '$lib/types/profile';
  import StepperInput from './StepperInput.svelte';
  import CollapseChevron from './CollapseChevron.svelte';
  import CollapseTab from './CollapseTab.svelte';

  type Props = {
    bezelHmm: number;
    bezelVmm: number;
    onBezelChange: (h: number, v: number) => void;
    fitMode: FitMode;
    onFitChange: (f: FitMode) => void;
    layoutMm: { w: number; h: number };
    monitorCount: number;
    totalPx: { w: number; h: number };
  };
  let {
    bezelHmm,
    bezelVmm,
    onBezelChange,
    fitMode,
    onFitChange,
    layoutMm,
    monitorCount,
    totalPx,
  }: Props = $props();

  const fits: FitMode[] = ['fill', 'fit', 'stretch', 'center'];
  let collapsed = $state(false);
</script>

{#if collapsed}
  <CollapseTab
    side="left"
    left={70}
    bottom={14}
    label="Bezels"
    summary="{bezelHmm.toFixed(1)} · {bezelVmm.toFixed(1)} mm"
    onExpand={() => (collapsed = false)}
  />
{:else}
  <div
    class="panel absolute flex items-center"
    style:left="70px"
    style:bottom="14px"
    style:padding="12px"
    style:padding-right="28px"
    style:gap="18px"
    style:z-index="5"
  >
    <div>
      <div class="section-label">Bezel gap</div>
      <div class="flex items-center" style:gap="8px">
        <StepperInput
          label="H"
          value={bezelHmm}
          unit="mm"
          step={0.5}
          bigStep={5}
          decimals={1}
          onChange={(v) => onBezelChange(v, bezelVmm)}
        />
        <StepperInput
          label="V"
          value={bezelVmm}
          unit="mm"
          step={0.5}
          bigStep={5}
          decimals={1}
          onChange={(v) => onBezelChange(bezelHmm, v)}
        />
      </div>
    </div>
    <div style:width="1px" style:height="36px" style:background="var(--line)"></div>
    <div>
      <div class="section-label">Fit</div>
      <div class="seg">
        {#each fits as f (f)}
          <button class:seg-active={fitMode === f} onclick={() => onFitChange(f)}>
            {f}
          </button>
        {/each}
      </div>
    </div>
    <div style:width="1px" style:height="36px" style:background="var(--line)"></div>
    <div>
      <div class="section-label">Layout</div>
      <div class="mono" style:font-size="11px" style:color="var(--text-2)">
        {monitorCount} mons ·
        <span style:color="var(--text)">{Math.round(layoutMm.w)}×{Math.round(layoutMm.h)}</span>
        mm · <span style:color="var(--text-3)">{totalPx.w}×{totalPx.h} px</span>
      </div>
    </div>
    <CollapseChevron side="left" onClick={() => (collapsed = true)} title="Collapse" />
  </div>
{/if}

<style>
  .section-label {
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-transform: uppercase;
    margin-bottom: 6px;
  }
  .seg {
    display: inline-flex;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .seg button {
    border: none;
    height: 26px;
    padding: 0 10px;
    font-size: 11px;
    font-weight: 500;
    background: transparent;
    color: var(--text-2);
    text-transform: capitalize;
  }
  .seg-active {
    background: var(--accent) !important;
    color: oklch(0.16 0.01 250) !important;
  }
</style>
