<script lang="ts">
  import StepperInput from './StepperInput.svelte';
  import CollapseChevron from './CollapseChevron.svelte';
  import CollapseTab from './CollapseTab.svelte';

  type Props = {
    hGapMm: number | null;
    vGapMm: number | null;
    hMixed: boolean;
    vMixed: boolean;
    fallbackHmm: number;
    fallbackVmm: number;
    onHGapChange: (h: number) => void;
    onVGapChange: (v: number) => void;
    layoutMm: { w: number; h: number };
    monitorCount: number;
    totalPx: { w: number; h: number };
  };
  let {
    hGapMm,
    vGapMm,
    hMixed,
    vMixed,
    fallbackHmm,
    fallbackVmm,
    onHGapChange,
    onVGapChange,
    layoutMm,
    monitorCount,
    totalPx,
  }: Props = $props();

  let collapsed = $state(false);

  const hValue = $derived(hGapMm ?? fallbackHmm);
  const vValue = $derived(vGapMm ?? fallbackVmm);
  const summary = $derived(
    `${hGapMm === null ? '—' : hGapMm.toFixed(1)} · ${vGapMm === null ? '—' : vGapMm.toFixed(1)} mm`,
  );
</script>

{#if collapsed}
  <CollapseTab
    side="left"
    left={70}
    bottom={14}
    label="Gaps"
    {summary}
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
      <div class="section-label">
        Monitor gap{#if hMixed || vMixed}
          <span
            class="mixed-tag"
            title="Adjacent monitor pairs disagree — committing a value normalises all pairs on that axis"
            >mixed</span
          >
        {/if}
      </div>
      <div class="flex items-center" style:gap="8px">
        <StepperInput
          label="H"
          value={hValue}
          unit="mm"
          step={0.5}
          bigStep={5}
          decimals={1}
          resetTo={0}
          onChange={onHGapChange}
        />
        <StepperInput
          label="V"
          value={vValue}
          unit="mm"
          step={0.5}
          bigStep={5}
          decimals={1}
          resetTo={0}
          onChange={onVGapChange}
        />
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
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .mixed-tag {
    font-size: 8.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--accent);
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 3px;
    background: color-mix(in oklab, var(--accent) 14%, transparent);
  }
</style>
