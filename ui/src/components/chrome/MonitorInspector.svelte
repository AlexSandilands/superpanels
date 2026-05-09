<script lang="ts">
  import type { PreviewMonitor } from '$lib/canvas/previewLayout';
  import { canvasView } from '$lib/stores/canvasView.svelte';
  import StepperInput from '../widgets/StepperInput.svelte';

  type ImageTransform = {
    offsetMmX: number;
    offsetMmY: number;
    widthMm: number;
    heightMm: number;
  };

  type Props = {
    monitor: PreviewMonitor;
    imageUrl: string | null;
    imageTransform: ImageTransform;
    onClose: () => void;
    onSetPrimary: () => void;
    onRotate: (delta: number) => void;
  };
  let { monitor, imageUrl, imageTransform, onClose, onSetPrimary, onRotate }: Props = $props();

  const cropInner = $derived({
    leftPct: ((imageTransform.offsetMmX - monitor.xMm) / monitor.wMm) * 100,
    topPct: ((imageTransform.offsetMmY - monitor.yMm) / monitor.hMm) * 100,
    widthPct: (imageTransform.widthMm / monitor.wMm) * 100,
    heightPct: (imageTransform.heightMm / monitor.hMm) * 100,
  });

  function setX(v: number) {
    canvasView.override(monitor.id, { xMm: v });
  }
  function setY(v: number) {
    canvasView.override(monitor.id, { yMm: v });
  }

  const diagInches = $derived(Math.sqrt(monitor.nativeWmm ** 2 + monitor.nativeHmm ** 2) / 25.4);
</script>

<div
  class="panel scroll absolute"
  style:right="14px"
  style:top="56px"
  style:width="300px"
  style:max-height="calc(100vh - 200px)"
  style:padding="14px"
  style:z-index="6"
>
  <div class="flex items-center" style:gap="8px" style:margin-bottom="12px">
    <span class="dot live"></span>
    <div style:font-size="13px" style:font-weight="600">{monitor.name}</div>
    {#if monitor.primary}
      <span class="chip active">PRIMARY</span>
    {/if}
    <button class="btn ghost icon sm" style:margin-left="auto" onclick={onClose} title="Close"
      >×</button
    >
  </div>

  <div class="section">
    <div class="section-label">Resolution &amp; rate</div>
    <div class="kv">
      <span>Mode</span>
      <span class="mono"
        >{monitor.pxW}×{monitor.pxH}{monitor.refreshHz
          ? ` @ ${monitor.refreshHz.toFixed(2)} Hz`
          : ''}</span
      >
    </div>
    <div class="kv">
      <span>Rotation</span>
      <span class="mono">{monitor.rotation}°</span>
    </div>
  </div>

  <div class="section">
    <div class="section-label">Physical size</div>
    {#if monitor.missing}
      <div style:font-size="11px" style:color="var(--warn)" style:margin-bottom="6px">
        Not provided — bezel math falls back to 96 DPI. Add it in Settings → Monitors.
      </div>
    {/if}
    <div class="kv">
      <span>Width</span>
      <span class="mono">{monitor.nativeWmm.toFixed(1)} mm</span>
    </div>
    <div class="kv">
      <span>Height</span>
      <span class="mono">{monitor.nativeHmm.toFixed(1)} mm</span>
    </div>
    <div class="kv">
      <span>Diagonal</span>
      <span class="mono">{diagInches.toFixed(1)}"</span>
    </div>
  </div>

  <div class="section">
    <div class="section-label">Crop on this screen</div>
    <div class="crop" style:aspect-ratio="{monitor.pxW} / {monitor.pxH}">
      {#if imageUrl}
        <div
          class="crop-img"
          style:left="{cropInner.leftPct}%"
          style:top="{cropInner.topPct}%"
          style:width="{cropInner.widthPct}%"
          style:height="{cropInner.heightPct}%"
          style:background-image="url({imageUrl})"
        ></div>
      {/if}
      <div class="crop-label mono">{monitor.pxW}×{monitor.pxH}</div>
    </div>
  </div>

  <div class="section">
    <div class="section-label">Position (mm, preview only)</div>
    <div class="flex" style:gap="8px">
      <StepperInput
        label="x"
        value={monitor.xMm}
        step={1}
        bigStep={10}
        decimals={0}
        min={Number.NEGATIVE_INFINITY}
        width={56}
        onChange={setX}
      />
      <StepperInput
        label="y"
        value={monitor.yMm}
        step={1}
        bigStep={10}
        decimals={0}
        min={Number.NEGATIVE_INFINITY}
        width={56}
        onChange={setY}
      />
    </div>
  </div>

  <div class="flex" style:gap="6px" style:margin-top="14px">
    <button class="btn sm" onclick={() => onRotate(-90)} title="[ rotate CCW">↺</button>
    <button class="btn sm" onclick={() => onRotate(90)} title="] rotate CW">↻</button>
    {#if !monitor.primary}
      <button class="btn sm" style:margin-left="auto" onclick={onSetPrimary}>Set as primary</button>
    {/if}
  </div>
</div>

<style>
  .section {
    margin-bottom: 14px;
  }
  .section-label {
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-transform: uppercase;
    margin-bottom: 6px;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    padding: 3px 0;
    font-size: 12px;
  }
  .kv > span:first-child {
    color: var(--text-3);
  }
  .crop {
    width: 100%;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 4px;
    position: relative;
    overflow: hidden;
  }
  .crop-img {
    position: absolute;
    background-size: 100% 100%;
    background-repeat: no-repeat;
    pointer-events: none;
  }
  .crop-label {
    position: absolute;
    bottom: 4px;
    right: 6px;
    font-size: 9px;
    color: oklch(1 0 0 / 0.7);
    text-shadow: 0 1px 2px oklch(0 0 0 / 0.6);
  }
</style>
