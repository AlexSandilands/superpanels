<script lang="ts">
  import type { Profile } from '$lib/api';
  import { profileThumbPath } from '$lib/profile-thumb';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { profileThumbs } from '$lib/stores/profile-thumbs.svelte';

  const PROFILE_BG = 'oklch(0.22 0 0)';

  type Props = {
    profiles: Profile[];
    activeName: string | null;
    slideshowPaused: boolean;
    slideshowCanPrev: boolean;
    onSwitch: (p: Profile) => void;
    onPrev: () => void;
    onNext: () => void;
    onTogglePause: () => void;
    onOpenSettings: () => void;
    onOpenWindow: () => void;
    onQuit: () => void;
    onClose: () => void;
  };
  let {
    profiles,
    activeName,
    slideshowPaused,
    slideshowCanPrev,
    onSwitch,
    onPrev,
    onNext,
    onTogglePause,
    onOpenSettings,
    onOpenWindow,
    onQuit,
    onClose,
  }: Props = $props();

  const active = $derived(profiles.find((p) => p.name === activeName) ?? null);
  const libraryPaths = $derived(libraryStore.entries.map((e) => e.path));

  function sourceLabel(p: Profile): string {
    if (p.body.type === 'standard') {
      const n = p.body.layers.length;
      if (n === 1) return p.body.layers[0]?.path || '(no image)';
      return `${n} image${n === 1 ? '' : 's'}`;
    }
    return 'slideshow';
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0" style:z-index="40" onclick={onClose}></div>

<div
  class="panel fixed"
  style:top="46px"
  style:right="18px"
  style:width="280px"
  style:padding="6px"
  style:z-index="41"
>
  <div style:padding="10px 12px" style:border-bottom="1px solid var(--line)">
    <div
      style:font-size="11px"
      style:color="var(--text-3)"
      style:font-weight="500"
      style:letter-spacing="0.04em"
    >
      SUPERPANELS
    </div>
    <div style:font-size="13px" style:font-weight="600" style:margin-top="2px">
      {active?.name ?? '— no profile'}
    </div>
    {#if active}
      <div class="mono" style:font-size="10px" style:color="var(--text-3)" style:margin-top="2px">
        {sourceLabel(active)}
      </div>
    {/if}
  </div>

  <div style:padding="6px 0">
    <div
      style:padding="4px 12px"
      style:font-size="9px"
      style:font-weight="600"
      style:letter-spacing="0.06em"
      style:color="var(--text-3)"
      style:text-transform="uppercase"
    >
      Profiles
    </div>
    {#each profiles as p (p.name)}
      {@const isActive = p.name === activeName}
      {@const thumb = profileThumbs.url(profileThumbPath(p, libraryPaths))}
      <button class="row" onclick={() => onSwitch(p)}>
        <span style:width="14px" style:color="var(--accent)" style:font-size="12px">
          {isActive ? '✓' : ''}
        </span>
        <span style:font-size="12px" style:flex="1" style:font-weight={isActive ? '600' : '400'}>
          {p.name}
        </span>
        <span
          class="swatch"
          style:background={thumb
            ? `center/cover no-repeat url("${thumb}"), ${PROFILE_BG}`
            : PROFILE_BG}
        ></span>
      </button>
    {/each}
    {#if profiles.length === 0}
      <div style:padding="8px 12px" style:font-size="11px" style:color="var(--text-3)">
        No profiles yet.
      </div>
    {/if}
  </div>

  <div class="divider" style:margin="4px 0"></div>
  <button class="row" disabled={!slideshowCanPrev} onclick={onPrev}>
    <span style:width="14px" style:color="var(--text-3)" style:font-size="11px">◀</span>
    <span>Previous wallpaper</span>
  </button>
  <button class="row" onclick={onTogglePause}>
    <span style:width="14px" style:color="var(--text-3)" style:font-size="11px">
      {slideshowPaused ? '▶' : '⏸'}
    </span>
    <span>{slideshowPaused ? 'Resume slideshow' : 'Pause slideshow'}</span>
  </button>
  <button class="row" onclick={onNext}>
    <span style:width="14px" style:color="var(--text-3)" style:font-size="11px">▶</span>
    <span>Next wallpaper</span>
  </button>
  <div class="divider" style:margin="4px 0"></div>
  <button class="row" onclick={onOpenWindow}>
    <span style:width="14px"></span>
    <span>Open Superpanels</span>
  </button>
  <button class="row" onclick={onOpenSettings}>
    <span style:width="14px"></span>
    <span>Settings…</span>
  </button>
  <button class="row row-danger" onclick={onQuit}>
    <span style:width="14px"></span>
    <span>Quit</span>
  </button>
</div>

<style>
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: inherit;
    text-align: left;
    font-size: 12px;
  }
  .row:hover:not(:disabled) {
    background: var(--panel-2);
  }
  .row:disabled {
    opacity: 0.4;
  }
  .row-danger {
    color: var(--danger);
  }
  .swatch {
    width: 22px;
    height: 14px;
    border-radius: 2px;
    border: 1px solid var(--line);
  }
</style>
