<script lang="ts">
  import type { Profile } from '$lib/api';
  import { profileThumbPath } from '$lib/profile-thumb';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { profileThumbs } from '$lib/stores/profile-thumbs.svelte';
  import Icon from '../widgets/Icon.svelte';

  const PROFILE_BG = 'oklch(0.22 0 0)';

  type Props = {
    profiles: Profile[];
    activeName: string | null;
    onPick: (p: Profile) => void;
    onClose: () => void;
    onOpenManager?: () => void;
    onTogglePauseSchedules?: () => void;
    schedulesPaused?: boolean;
    activeScheduleHint?: string | null;
  };
  let {
    profiles,
    activeName,
    onPick,
    onClose,
    onOpenManager,
    onTogglePauseSchedules,
    schedulesPaused = false,
    activeScheduleHint = null,
  }: Props = $props();

  function modeLabel(p: Profile): string {
    return p.body.type === 'span' ? 'span' : 'per-monitor';
  }

  function sourceLabel(p: Profile): string {
    if (p.body.type === 'span') {
      const src = p.body.source;
      if (src.type === 'single') return src.path || '(no image)';
      return 'slideshow';
    }
    return `${p.body.assignments.length} pins`;
  }

  // Sort: pinned/active first, then by last_applied_at desc.
  let sortedProfiles = $derived(
    [...profiles].sort((a, b) => {
      if (a.name === activeName) return -1;
      if (b.name === activeName) return 1;
      const ta = a.last_applied_at ? new Date(a.last_applied_at).getTime() : 0;
      const tb = b.last_applied_at ? new Date(b.last_applied_at).getTime() : 0;
      return tb - ta;
    }),
  );

  const libraryPaths = $derived(libraryStore.entries.map((e) => e.path));

  // Outside-click + Escape dismiss. The popover sits inside the title-bar's
  // z-index:10 stacking context, so a fixed-position shield wouldn't reliably
  // intercept canvas clicks; a window-level pointerdown listener bound to the
  // panel's bbox dodges that entirely.
  let panelEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    function onDocDown(e: PointerEvent) {
      if (panelEl && panelEl.contains(e.target as Node)) return;
      onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose();
    }
    // Defer one tick so the click that opens the menu doesn't immediately close it.
    const id = window.setTimeout(() => {
      window.addEventListener('pointerdown', onDocDown, true);
    }, 0);
    window.addEventListener('keydown', onKey);
    return () => {
      window.clearTimeout(id);
      window.removeEventListener('pointerdown', onDocDown, true);
      window.removeEventListener('keydown', onKey);
    };
  });
</script>

<div
  bind:this={panelEl}
  class="panel absolute"
  style:top="32px"
  style:left="0"
  style:width="280px"
  style:padding="6px"
  style:z-index="21"
>
  <div
    style:padding="6px 8px"
    style:font-size="10px"
    style:font-weight="600"
    style:letter-spacing="0.06em"
    style:color="var(--text-3)"
    style:text-transform="uppercase"
  >
    Profiles
  </div>
  {#if activeScheduleHint}
    <div class="hint">{activeScheduleHint}</div>
  {/if}
  {#each sortedProfiles as p, i (p.name)}
    {@const isActive = p.name === activeName}
    {@const thumb = profileThumbs.url(profileThumbPath(p, libraryPaths))}
    <button
      class="profile-row"
      class:profile-row--active={isActive}
      onclick={() => onPick(p)}
      title={p.name}
    >
      <span
        class="swatch"
        style:background={thumb
          ? `center/cover no-repeat url("${thumb}"), ${PROFILE_BG}`
          : PROFILE_BG}
      ></span>
      <div style:flex="1" style:min-width="0">
        <div class="name" style:font-weight={isActive ? '600' : '500'}>
          {p.name}
        </div>
        <div class="sub mono">
          {modeLabel(p)} · {sourceLabel(p)}
        </div>
      </div>
      {#if i < 3}
        <span class="kbd">⌃{i + 1}</span>
      {/if}
    </button>
  {/each}
  {#if profiles.length === 0}
    <div class="empty">
      <p>No profiles yet — create one below.</p>
    </div>
  {/if}
  <div class="divider"></div>
  {#if onOpenManager}
    <button class="btn ghost full" onclick={onOpenManager}>
      <Icon name="grid" /> Open profile manager…
    </button>
  {/if}
  {#if onTogglePauseSchedules}
    <button class="btn ghost full" onclick={onTogglePauseSchedules}>
      {schedulesPaused ? 'Resume schedules' : 'Pause schedules'}
    </button>
  {/if}
</div>

<style>
  .profile-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: inherit;
    text-align: left;
  }
  .profile-row:hover {
    background: var(--panel-2);
  }
  .profile-row--active {
    background: color-mix(in oklab, var(--accent) 15%, transparent);
  }
  .name {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 10px;
    color: var(--text-3);
    margin-top: 1px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .swatch {
    width: 28px;
    height: 18px;
    border-radius: 3px;
    border: 1px solid var(--line);
    flex-shrink: 0;
  }
  .full {
    width: 100%;
    justify-content: flex-start;
  }
  .empty {
    padding: 10px;
    font-size: 11px;
    color: var(--text-3);
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: stretch;
  }
  .empty p {
    margin: 0;
  }
  .hint {
    margin: 4px 6px;
    padding: 6px 8px;
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-2);
  }
</style>
