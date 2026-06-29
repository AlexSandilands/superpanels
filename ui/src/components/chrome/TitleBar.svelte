<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { Profile } from '$lib/api';
  import Icon from '../widgets/Icon.svelte';
  import WindowControls from './WindowControls.svelte';
  import ProfileMenu from './ProfileMenu.svelte';
  import WindowMenu from './WindowMenu.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';

  type Props = {
    profiles: Profile[];
    activeName: string | null;
    backendName: string;
    canApply: boolean;
    canSaveAsNew: boolean;
    canSave: boolean;
    canRevert: boolean;
    saveDirty: boolean;
    onApply: () => void;
    onSave: () => void;
    onSaveAsNew: () => void;
    onRevert: () => void;
    onSwitchProfile: (p: Profile) => void;
    onOpenLibrary: () => void;
    onOpenSettings: () => void;
    onOpenProfileManager: () => void;
    onTrayClick: () => void;
  };
  let {
    profiles,
    activeName,
    backendName,
    canApply,
    canSaveAsNew,
    canSave,
    canRevert,
    saveDirty,
    onApply,
    onSave,
    onSaveAsNew,
    onRevert,
    onSwitchProfile,
    onOpenLibrary,
    onOpenSettings,
    onOpenProfileManager,
    onTrayClick,
  }: Props = $props();

  let menuOpen = $state(false);
  let nowMs = $state(Date.now());

  let winMenu = $state<{ x: number; y: number } | null>(null);
  let isMaximized = $state(false);
  let alwaysOnTop = $state(false);

  $effect(() => {
    const id = window.setInterval(() => (nowMs = Date.now()), 1000);
    return () => window.clearInterval(id);
  });

  $effect(() => {
    const w = (() => {
      try {
        return getCurrentWindow();
      } catch {
        return null;
      }
    })();
    if (!w) return;
    void w.isMaximized().then((v) => (isMaximized = v));
    const unlisten = w.onResized(() => {
      void w.isMaximized().then((v) => (isMaximized = v));
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  function onTitlebarContextMenu(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button, input, select, textarea, [role="menu"]')) return;
    e.preventDefault();
    winMenu = { x: e.clientX, y: e.clientY };
  }

  function onTitlebarMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    if (e.detail >= 2) return; // let dblclick fire for maximize-toggle
    if ((e.target as HTMLElement).closest('button, input, select, textarea, [role="menu"]')) return;
    void getCurrentWindow()
      .startDragging()
      .catch((err) => {
        // eslint-disable-next-line no-console -- reason: surface startDragging failures to devtools so we can tell if it's rejecting (e.g. Wayland serial issue) vs. the handler not firing at all.
        console.warn('[titlebar] startDragging failed', err);
      });
  }

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
  role="toolbar"
  aria-label="Window titlebar"
  tabindex="-1"
  onmousedown={onTitlebarMouseDown}
  oncontextmenu={onTitlebarContextMenu}
  ondblclick={(e) => {
    if ((e.target as HTMLElement).closest('button, input, select, textarea')) return;
    void getCurrentWindow()
      .toggleMaximize()
      .catch(() => {});
  }}
>
  <div class="flex items-center" style:gap="8px" style:margin-right="6px">
    <Icon name="logo" size={20} />
    <span style:font-weight="600" style:font-size="13px" style:letter-spacing="-0.01em"
      >Superpanels</span
    >
  </div>

  <div style:width="1px" style:height="18px" style:background="var(--line)"></div>

  <div class="relative">
    <button
      class="btn"
      style:height="26px"
      style:font-size="12px"
      onclick={() => (menuOpen = !menuOpen)}
    >
      <span class="dot live"></span>
      <span style:font-weight="600">
        {activeProfile?.name ?? '— no profile'}
      </span>
      {#if activeProfile}
        <span style:color="var(--text-3)" style:font-size="11px">
          {activeProfile.body.type}
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
        onOpenManager={() => {
          menuOpen = false;
          onOpenProfileManager();
        }}
      />
    {/if}
  </div>

  <div style:flex="1"></div>

  <div class="flex items-center" style:gap="8px">
    <span class="chip" title="Last apply">
      <span class="dot ok"></span>
      <span class="mono" style:color="var(--text-2)">{backendName}</span>
      <span style:color="var(--text-3)">·</span>
      <span class="mono" style:color="var(--text-3)">{lastApplyText}</span>
    </span>
    <!-- Content: library + profiles -->
    <button class="btn ghost icon" title="Library (Ctrl+L)" onclick={onOpenLibrary}>
      <Icon name="grid" />
    </button>
    <button class="btn ghost icon" title="Profile manager" onclick={onOpenProfileManager}>
      <Icon name="stack" />
    </button>
    <div style:width="1px" style:height="18px" style:background="var(--line)"></div>
    <!-- Canvas actions: revert, save, save-as-new -->
    <button
      class="btn ghost icon"
      disabled={!canRevert}
      onclick={onRevert}
      title={canRevert
        ? `Revert canvas to '${activeName}' (drops unsaved edits)`
        : 'Nothing to revert'}
    >
      <Icon name="reset" />
    </button>
    <button
      class="btn ghost icon"
      class:save-dirty={saveDirty}
      disabled={!canSave}
      onclick={onSave}
      title={canSave
        ? saveDirty
          ? `Save changes to '${activeName}' (Ctrl+S)`
          : `Save '${activeName}' (no changes)`
        : 'No active profile to save'}
    >
      <Icon name="save" />
    </button>
    <button
      class="btn ghost icon"
      disabled={!canSaveAsNew}
      onclick={onSaveAsNew}
      title={canSaveAsNew ? 'Save current canvas as a new profile' : 'No image on canvas'}
    >
      <Icon name="save-new" />
    </button>
    <div style:width="1px" style:height="18px" style:background="var(--line)"></div>
    <!-- Utilities: settings, tray -->
    <button class="btn ghost icon" title="Settings (Ctrl+,)" onclick={onOpenSettings}>
      <Icon name="gear" />
    </button>
    <button class="btn ghost icon" title="Tray menu" onclick={onTrayClick}>
      <Icon name="tray" />
    </button>
    <div style:width="1px" style:height="18px" style:background="var(--line)"></div>
    <button class="btn primary" disabled={!canApply} onclick={onApply} title="Apply (Enter)">
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

{#if winMenu}
  <WindowMenu
    x={winMenu.x}
    y={winMenu.y}
    {isMaximized}
    {alwaysOnTop}
    onClose={() => (winMenu = null)}
    onAlwaysOnTopChange={(v) => (alwaysOnTop = v)}
  />
{/if}

<style>
  /* Dirty Save: tint the icon button with --accent so the user can tell
     at a glance there are unsaved canvas edits (§9.1.2 / §12.4.3). */
  .save-dirty {
    color: var(--accent);
  }
</style>
