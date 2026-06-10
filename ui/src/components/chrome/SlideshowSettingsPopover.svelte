<script lang="ts">
  import { portal } from '$lib/portal';
  import type { SlideshowConfig } from '$lib/types/SlideshowConfig';
  import type { SlideshowSort } from '$lib/types/SlideshowSort';
  import type { SlideshowStart } from '$lib/types/SlideshowStart';
  import Toggle from '../overlays/settings/Toggle.svelte';
  import Select from '../widgets/Select.svelte';
  import StepperInput from '../widgets/StepperInput.svelte';
  import Icon from '../widgets/Icon.svelte';

  type Props = {
    /** Element the popover hangs above; also fixes its right edge. */
    anchor: HTMLElement;
    config: SlideshowConfig;
    onChange: (config: SlideshowConfig) => void;
    onManageImages: () => void;
    onClose: () => void;
  };
  let { anchor, config, onChange, onManageImages, onClose }: Props = $props();

  // Position is computed once at open — the popover is remounted per open,
  // so tracking later anchor movement is unnecessary.
  // svelte-ignore state_referenced_locally
  const rect = anchor.getBoundingClientRect();
  const right = window.innerWidth - rect.right;
  const bottom = window.innerHeight - rect.top + 10;

  const sortOptions = [
    { value: 'shuffle', label: 'Shuffle' },
    { value: 'alphabetical', label: 'A → Z' },
    { value: 'date_asc', label: 'Oldest first' },
    { value: 'date_desc', label: 'Newest first' },
    { value: 'last_shown_asc', label: 'Least recently shown' },
  ];
  const startOptions = [
    { value: 'resume', label: 'Resume where it left off' },
    { value: 'new_random', label: 'Start fresh' },
    { value: 'first', label: 'First image' },
  ];
  const presets = [
    { label: '1m', secs: 60 },
    { label: '5m', secs: 300 },
    { label: '15m', secs: 900 },
    { label: '30m', secs: 1800 },
    { label: '1h', secs: 3600 },
    { label: '4h', secs: 14_400 },
  ];

  const intervalMins = $derived(Math.max(1, Math.round(config.interval_secs / 60)));

  function patch(partial: Partial<SlideshowConfig>) {
    onChange({ ...config, ...partial });
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div use:portal class="fixed inset-0" style:z-index="40" onclick={onClose}></div>
<div
  use:portal
  class="panel pop"
  role="dialog"
  aria-label="Slideshow settings"
  style:right="{right}px"
  style:bottom="{bottom}px"
>
  <div class="title">Slideshow settings</div>

  <div class="row">
    <span class="lbl">Every</span>
    <StepperInput
      value={intervalMins}
      unit="min"
      step={1}
      bigStep={10}
      min={1}
      max={10_080}
      decimals={0}
      width={48}
      onChange={(v) => patch({ interval_secs: Math.max(60, Math.round(v) * 60) })}
    />
  </div>
  <div class="presets">
    {#each presets as p (p.secs)}
      <button
        class="chip"
        class:active={config.interval_secs === p.secs}
        onclick={() => patch({ interval_secs: p.secs })}
      >
        {p.label}
      </button>
    {/each}
  </div>

  <div class="row">
    <span class="lbl">Order</span>
    <Select
      value={config.sort}
      options={sortOptions}
      minWidth={150}
      onChange={(v) => patch({ sort: v as SlideshowSort })}
    />
  </div>
  <div class="row">
    <span class="lbl">On start</span>
    <Select
      value={config.on_start}
      options={startOptions}
      minWidth={150}
      onChange={(v) => patch({ on_start: v as SlideshowStart })}
    />
  </div>
  <div class="row">
    <span class="lbl">Avoid repeating last</span>
    <StepperInput
      value={config.recent_history_size}
      unit="img"
      step={1}
      bigStep={10}
      min={0}
      max={500}
      decimals={0}
      width={44}
      onChange={(v) => patch({ recent_history_size: Math.max(0, Math.round(v)) })}
    />
  </div>
  <div class="row">
    <span class="lbl">Skip missing files</span>
    <Toggle
      value={config.skip_on_unavailable}
      onChange={(v) => patch({ skip_on_unavailable: v })}
    />
  </div>
  <div class="row">
    <span class="lbl">Pause while app is open</span>
    <Toggle value={config.pause_when_active} onChange={(v) => patch({ pause_when_active: v })} />
  </div>

  <div class="footer">
    <button
      class="btn sm"
      style:width="100%"
      onclick={() => {
        onClose();
        onManageImages();
      }}
    >
      <Icon name="grid" size={12} /> Manage images…
    </button>
  </div>
</div>

<style>
  .pop {
    position: fixed;
    width: 264px;
    padding: 12px 14px;
    z-index: 41;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
    margin-bottom: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-height: 24px;
  }
  .lbl {
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .presets {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .footer {
    margin-top: 4px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
</style>
