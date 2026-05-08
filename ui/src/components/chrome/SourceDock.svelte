<script lang="ts">
  import Icon from '../Icon.svelte';
  import CollapseChevron from './CollapseChevron.svelte';
  import CollapseTab from './CollapseTab.svelte';

  type Slideshow = { paused: boolean; index: number; total: number } | null;

  type Props = {
    sourceName: string;
    sourceMeta: string;
    sourceThumbUrl: string | null;
    slideshow: Slideshow;
    onPrev: () => void;
    onNext: () => void;
    onTogglePause: () => void;
    onOpenLibrary: () => void;
  };
  let {
    sourceName,
    sourceMeta,
    sourceThumbUrl,
    slideshow,
    onPrev,
    onNext,
    onTogglePause,
    onOpenLibrary,
  }: Props = $props();

  let collapsed = $state(false);

  // Stack above BezelDock when the window is narrow enough that the two would
  // otherwise overlap. Mirrors the ModeHint pattern so the bottom row degrades
  // predictably as width shrinks.
  const STACK_BREAKPOINT = 1180;
  let innerWidth = $state(typeof window === 'undefined' ? 1920 : window.innerWidth);

  $effect(() => {
    const onResize = () => (innerWidth = window.innerWidth);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  const stacked = $derived(innerWidth < STACK_BREAKPOINT);
  const bottomPx = $derived(stacked ? 96 : 14);

  const tabSummary = $derived(
    slideshow && slideshow.total > 0
      ? `${sourceName} · ${slideshow.index + 1}/${slideshow.total}`
      : sourceName,
  );
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
      style:width="56px"
      style:height="32px"
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
      <div class="mono" style:font-size="10px" style:color="var(--text-3)">
        {slideshow.total > 0 ? `${slideshow.index + 1}/${slideshow.total}` : '—'}
      </div>
    {/if}
    <div style:width="1px" style:height="28px" style:background="var(--line)"></div>
    <button class="btn sm" onclick={onOpenLibrary}>
      <Icon name="grid" size={12} /> Library
    </button>
  </div>
{/if}
