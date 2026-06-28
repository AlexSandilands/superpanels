<script lang="ts">
  import type { SlideshowState } from '$lib/actions';
  import { dockStackBreakpoint } from '$lib/dock-breakpoints';
  import type { SlideshowConfig } from '$lib/types/SlideshowConfig';
  import Icon from '../widgets/Icon.svelte';
  import CollapseChevron from './CollapseChevron.svelte';
  import CollapseTab from './CollapseTab.svelte';
  import SlideshowSettingsPopover from './SlideshowSettingsPopover.svelte';
  import SlideshowJumpPopover from './SlideshowJumpPopover.svelte';

  type Props = {
    sourceName: string;
    sourceMeta: string;
    sourceThumbUrl: string | null;
    /** Set images, for the quick-jump grid. */
    jumpImages: string[];
    /** The live image path, highlighted in the jump grid. */
    currentImagePath: string | null;
    onJump: (path: string) => void;
    slideshow: SlideshowState;
    slideshowConfig: SlideshowConfig | null;
    /** A live slideshow image is up — the canvas can be saved for it. */
    canSaveForImage: boolean;
    /** The live image already has a per-image canvas override. */
    hasImageOverride: boolean;
    /** The slideshow applies one profile-level layout to every image. */
    uniformLayoutOn: boolean;
    onPrev: () => void;
    onNext: () => void;
    onTogglePause: () => void;
    onUpdateConfig: (config: SlideshowConfig) => void;
    onSaveForImage: () => void;
    onResetForImage: () => void;
    onApplyToAll: () => void;
    onResetUniform: () => void;
    onOpenLibrary: () => void;
  };
  let {
    sourceName,
    sourceMeta,
    sourceThumbUrl,
    jumpImages,
    currentImagePath,
    onJump,
    slideshow,
    slideshowConfig,
    canSaveForImage,
    hasImageOverride,
    uniformLayoutOn,
    onPrev,
    onNext,
    onTogglePause,
    onUpdateConfig,
    onSaveForImage,
    onResetForImage,
    onApplyToAll,
    onResetUniform,
    onOpenLibrary,
  }: Props = $props();

  let collapsed = $state(false);
  let settingsOpen = $state(false);
  let jumpOpen = $state(false);
  let gearEl: HTMLButtonElement | undefined = $state();

  // Stack above MonitorGapDock when the window is narrow enough that the two
  // would otherwise overlap. Mirrors the ModeHint pattern so the bottom row
  // degrades predictably as width shrinks. Slideshow chrome widens the dock,
  // so its breakpoint sits higher — see dockStackBreakpoint.
  let innerWidth = $state(typeof window === 'undefined' ? 1920 : window.innerWidth);

  $effect(() => {
    const onResize = () => (innerWidth = window.innerWidth);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  const stacked = $derived(innerWidth < dockStackBreakpoint(Boolean(slideshow || slideshowConfig)));
  const bottomPx = $derived(stacked ? 96 : 14);

  // Countdown ticks locally between runtime refreshes off the daemon's
  // remaining-seconds snapshot.
  let nowMs = $state(Date.now());
  $effect(() => {
    if (!slideshow || slideshow.paused || slideshow.remainingSecs === null) return;
    const id = window.setInterval(() => (nowMs = Date.now()), 1000);
    return () => window.clearInterval(id);
  });
  const countdownSecs = $derived.by(() => {
    if (!slideshow || slideshow.paused || slideshow.remainingSecs === null) return null;
    // A runtime refresh can land between local ticks, putting `fetchedAt`
    // ahead of `nowMs` — clamp so the countdown never exceeds the snapshot.
    const elapsed = Math.max(0, Math.floor((nowMs - slideshow.fetchedAt) / 1000));
    return Math.max(0, slideshow.remainingSecs - elapsed);
  });

  function fmtCountdown(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  const counterText = $derived.by(() => {
    if (!slideshow || slideshow.total <= 0) return '—';
    // Under shuffle the pool position jumps randomly, and after Prev it is
    // unknown — show only the pool size instead of a misleading "n/m".
    if (slideshow.index === null || slideshowConfig?.sort === 'shuffle') {
      return `${slideshow.total} imgs`;
    }
    return `${slideshow.index + 1}/${slideshow.total}`;
  });

  const tabSummary = $derived(
    slideshow && slideshow.total > 0 ? `${sourceName} · ${counterText}` : sourceName,
  );

  const shuffleOn = $derived(slideshowConfig?.sort === 'shuffle');

  function toggleShuffle() {
    if (!slideshowConfig) return;
    onUpdateConfig({
      ...slideshowConfig,
      sort: shuffleOn ? 'alphabetical' : 'shuffle',
    });
  }
</script>

{#if collapsed}
  <CollapseTab
    side="right"
    right={14}
    bottom={bottomPx}
    label="Source"
    summary={tabSummary}
    onExpand={() => (collapsed = false)}
  />
{:else}
  <div
    class="panel absolute flex items-center"
    style:right="14px"
    style:bottom="{bottomPx}px"
    style:padding="8px"
    style:padding-left="28px"
    style:gap="10px"
    style:z-index="5"
    style:transition="bottom 140ms ease"
  >
    <CollapseChevron side="right" onClick={() => (collapsed = true)} title="Collapse" />
    <div
      style:width="76px"
      style:height="44px"
      style:border-radius="4px"
      style:border="1px solid var(--line)"
      style:flex-shrink="0"
      style:background={sourceThumbUrl
        ? `center / cover url(${sourceThumbUrl})`
        : 'linear-gradient(135deg, var(--bg-2), var(--panel-2))'}
    ></div>
    <div style:min-width="140px" style:max-width="200px">
      <div
        style:font-size="11px"
        style:font-weight="600"
        style:overflow="hidden"
        style:text-overflow="ellipsis"
        style:white-space="nowrap"
      >
        {sourceName}
      </div>
      <div class="mono" style:font-size="10px" style:color="var(--text-3)" style:margin-top="2px">
        {sourceMeta}
      </div>
    </div>
    {#if slideshow}
      <div style:width="1px" style:height="28px" style:background="var(--line)"></div>
      <div class="flex items-center" style:gap="2px">
        <button class="btn ghost icon sm" title="Previous (←)" onclick={onPrev}>
          <Icon name="prev" size={12} />
        </button>
        <button class="btn ghost icon sm" title="Pause/resume (Space)" onclick={onTogglePause}>
          <Icon name={slideshow.paused ? 'play' : 'pause'} size={12} />
        </button>
        <button class="btn ghost icon sm" title="Next (→)" onclick={onNext}>
          <Icon name="next" size={12} />
        </button>
      </div>
      {#if canSaveForImage}
        <div class="flex items-center" style:gap="2px">
          <button
            class="btn ghost icon sm"
            class:override-on={hasImageOverride}
            title={hasImageOverride
              ? 'This image has a custom layout — save the canvas over it'
              : 'Save the current canvas (gaps + image position) for this image only'}
            onclick={onSaveForImage}
          >
            <Icon name="save" size={12} />
          </button>
          {#if hasImageOverride}
            <button
              class="btn ghost icon sm"
              title="Remove this image's custom layout (back to the profile layout)"
              onclick={onResetForImage}
            >
              <Icon name="reset" size={12} />
            </button>
          {/if}
          <button
            class="btn ghost icon sm"
            class:override-on={uniformLayoutOn}
            title={uniformLayoutOn
              ? 'Uniform layout is on — re-save the current canvas as the layout for all images'
              : 'Apply the current canvas (gaps + image position) to all images in the slideshow'}
            onclick={onApplyToAll}
          >
            <Icon name="stack" size={12} />
          </button>
          {#if uniformLayoutOn}
            <button
              class="btn ghost icon sm"
              title="Turn off uniform layout (auto-fit each image to the monitors)"
              onclick={onResetUniform}
            >
              <Icon name="fit" size={12} />
            </button>
          {/if}
        </div>
      {/if}
      <div class="jump-anchor">
        <button
          class="mono counter counter-btn"
          style:font-size="10px"
          disabled={jumpImages.length === 0}
          title={jumpImages.length ? 'Jump to an image in the set' : undefined}
          onclick={() => (jumpOpen = !jumpOpen)}
        >
          {counterText}
          {#if slideshow.paused}
            <span class="mono" style:color="var(--warn)">paused</span>
          {:else if countdownSecs !== null}
            <span class="mono countdown" title="Time until next wallpaper">
              {fmtCountdown(countdownSecs)}
            </span>
          {/if}
        </button>
        {#if jumpOpen}
          <SlideshowJumpPopover
            images={jumpImages}
            current={currentImagePath}
            {onJump}
            onClose={() => (jumpOpen = false)}
          />
        {/if}
      </div>
    {/if}
    {#if slideshowConfig}
      <div style:width="1px" style:height="28px" style:background="var(--line)"></div>
      <div class="flex items-center" style:gap="2px">
        <button
          class="btn ghost icon sm"
          class:shuffle-on={shuffleOn}
          title={shuffleOn ? 'Shuffle on — switch to A → Z' : 'Shuffle off — switch to shuffle'}
          onclick={toggleShuffle}
        >
          <Icon name="shuffle" size={12} />
        </button>
        <button
          bind:this={gearEl}
          class="btn ghost icon sm"
          title="Slideshow settings"
          aria-expanded={settingsOpen}
          onclick={() => (settingsOpen = !settingsOpen)}
        >
          <Icon name="gear" size={12} />
        </button>
        {#if settingsOpen && gearEl}
          <SlideshowSettingsPopover
            anchor={gearEl}
            config={slideshowConfig}
            onChange={onUpdateConfig}
            onManageImages={onOpenLibrary}
            onClose={() => (settingsOpen = false)}
          />
        {/if}
      </div>
    {/if}
    <div style:width="1px" style:height="28px" style:background="var(--line)"></div>
    <button class="btn sm" onclick={onOpenLibrary}>
      <Icon name="grid" size={12} /> Library
    </button>
  </div>
{/if}

<style>
  .counter {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    min-width: 38px;
  }
  .jump-anchor {
    position: relative;
  }
  .counter-btn {
    appearance: none;
    border: none;
    background: none;
    padding: 2px 4px;
    border-radius: 4px;
    color: var(--text-3);
    cursor: pointer;
    transition: background 80ms;
  }
  .counter-btn:hover:not(:disabled) {
    background: var(--bg-2);
    color: var(--text-2);
  }
  .counter-btn:disabled {
    cursor: default;
  }
  .countdown {
    font-size: 10px;
    color: var(--accent);
  }
  .shuffle-on {
    color: var(--accent);
  }
  .override-on {
    color: var(--accent);
  }
</style>
