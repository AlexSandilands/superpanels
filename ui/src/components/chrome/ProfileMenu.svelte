<script lang="ts">
  import type { Profile } from '$lib/api';
  import { profileSwatch } from '$lib/profile-swatch';
  import Icon from '../widgets/Icon.svelte';

  import { toast } from '$lib/stores/toast.svelte';

  type Props = {
    profiles: Profile[];
    activeName: string | null;
    onPick: (p: Profile) => void;
    onClose: () => void;
    onNewProfile: () => void;
  };
  let { profiles, activeName, onPick, onClose, onNewProfile }: Props = $props();

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
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0" style:z-index="20" onclick={onClose}></div>
<div
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
  {#each profiles as p, i (p.name)}
    {@const isActive = p.name === activeName}
    <button class="profile-row" class:profile-row--active={isActive} onclick={() => onPick(p)}>
      <span class="swatch" style:background={profileSwatch(p.name)}></span>
      <div style:flex="1" style:min-width="0">
        <div
          style:font-size="12px"
          style:font-weight={isActive ? '600' : '500'}
          style:display="flex"
          style:gap="6px"
          style:align-items="center"
        >
          {p.name}
        </div>
        <div class="mono" style:font-size="10px" style:color="var(--text-3)" style:margin-top="1px">
          {modeLabel(p)} · {sourceLabel(p)}
        </div>
      </div>
      {#if i < 3}
        <span class="kbd">⌃{i + 1}</span>
      {/if}
    </button>
  {/each}
  {#if profiles.length === 0}
    <div style:padding="10px" style:font-size="11px" style:color="var(--text-3)">
      No profiles yet.
    </div>
  {/if}
  <div class="divider"></div>
  <button
    class="btn ghost"
    style:width="100%"
    style:justify-content="flex-start"
    onclick={onNewProfile}
  >
    <Icon name="plus" /> New profile <span class="kbd" style:margin-left="auto">⌃N</span>
  </button>
  <button
    class="btn ghost"
    style:width="100%"
    style:justify-content="flex-start"
    onclick={() => toast.info('Save current as new', 'not yet wired in this build')}
  >
    Save current as new <span class="kbd" style:margin-left="auto">⌃⇧S</span>
  </button>
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
  .profile-row--active:hover {
    background: color-mix(in oklab, var(--accent) 18%, transparent);
  }
  .swatch {
    width: 28px;
    height: 18px;
    border-radius: 3px;
    border: 1px solid var(--line);
    flex-shrink: 0;
  }
</style>
