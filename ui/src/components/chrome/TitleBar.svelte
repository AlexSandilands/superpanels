<script lang="ts">
  import type { Profile } from '$lib/api';
  import Icon from '../Icon.svelte';
  import WindowControls from './WindowControls.svelte';
  import ProfileMenu from './ProfileMenu.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';

  type Props = {
    profiles: Profile[];
    activeName: string | null;
    backendName: string;
    canApply: boolean;
    onApply: () => void;
    onSwitchProfile: (p: Profile) => void;
    onNewProfile: () => void;
    onOpenLibrary: () => void;
    onOpenSettings: () => void;
    onTrayClick: () => void;
  };
  let {
    profiles,
    activeName,
    backendName,
    canApply,
    onApply,
    onSwitchProfile,
    onNewProfile,
    onOpenLibrary,
    onOpenSettings,
    onTrayClick,
  }: Props = $props();

  let menuOpen = $state(false);
  let nowMs = $state(Date.now());

  $effect(() => {
    const id = window.setInterval(() => (nowMs = Date.now()), 1000);
    return () => window.clearInterval(id);
  });

  const lastApplyText = $derived(runtime.describeLastApply(nowMs));
  const activeProfile = $derived(profiles.find((p) => p.name === activeName) ?? null);
</script>

<div
  class="absolute left-0 right-0 top-0 z-10 flex items-center"
  style:height="40px"
  style:padding="0 12px"
  style:gap="10px"
  style:background="color-mix(in oklab, var(--bg) 70%, transparent)"
  style:border-bottom="1px solid var(--line)"
  data-tauri-drag-region
>
  <div class="flex items-center" style:gap="8px" style:margin-right="6px">
    <Icon name="logo" size={20} />
    <span style:font-weight="600" style:font-size="13px" style:letter-spacing="-0.01em"
      >Superpanels</span
    >
  </div>

  <div style:width="1px" style:height="18px" style:background="var(--line)"></div>

  <div class="relative" data-tauri-drag-region="false">
    <button
      class="btn"
      style:height="26px"
      style:font-size="12px"
      onclick={() => (menuOpen = !menuOpen)}
      data-tauri-drag-region="false"
    >
      <span class="dot live"></span>
      <span style:font-weight="600">
        {activeProfile?.name ?? '— no profile'}
      </span>
      {#if activeProfile}
        <span style:color="var(--text-3)" style:font-size="11px">
          {activeProfile.body.type === 'span' ? 'span' : 'per-monitor'}
        </span>
      {/if}
      <Icon name="caret" size={10} />
    </button>
    {#if menuOpen}
      <ProfileMenu
        {profiles}
        {activeName}
        onPick={(p) => {
          menuOpen = false;
          onSwitchProfile(p);
        }}
        onClose={() => (menuOpen = false)}
        onNewProfile={() => {
          menuOpen = false;
          onNewProfile();
        }}
      />
    {/if}
  </div>

  <div style:flex="1"></div>

  <div class="flex items-center" style:gap="8px" data-tauri-drag-region="false">
    <span class="chip" title="Last apply" data-tauri-drag-region="false">
      <span class="dot ok"></span>
      <span class="mono" style:color="var(--text-2)">{backendName}</span>
      <span style:color="var(--text-3)">·</span>
      <span class="mono" style:color="var(--text-3)">{lastApplyText}</span>
    </span>
    <button
      class="btn ghost icon"
      title="Library (Ctrl+L)"
      onclick={onOpenLibrary}
      data-tauri-drag-region="false"
    >
      <Icon name="grid" />
    </button>
    <button
      class="btn ghost icon"
      title="Settings (Ctrl+,)"
      onclick={onOpenSettings}
      data-tauri-drag-region="false"
    >
      <Icon name="gear" />
    </button>
    <button
      class="btn ghost icon"
      title="Tray menu"
      onclick={onTrayClick}
      data-tauri-drag-region="false"
    >
      <Icon name="tray" />
    </button>
    <div style:width="1px" style:height="18px" style:background="var(--line)"></div>
    <button
      class="btn primary"
      disabled={!canApply}
      onclick={onApply}
      title="Apply (Enter)"
      data-tauri-drag-region="false"
    >
      <Icon name="check" size={13} /> Apply
      <span
        class="kbd"
        style:margin-left="4px"
        style:background="oklch(0 0 0 / 0.18)"
        style:border-color="oklch(0 0 0 / 0.2)"
        style:color="oklch(0.18 0.01 250)"
      >
        ↵
      </span>
    </button>
    <div style:width="1px" style:height="18px" style:background="var(--line)"></div>
    <WindowControls />
  </div>
</div>
