<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import Sidebar from './components/Sidebar.svelte';
  import MonitorCanvas from './components/MonitorCanvas.svelte';
  import ProfileList from './components/ProfileList.svelte';
  import ProfileEditor from './components/ProfileEditor.svelte';
  import LibraryPane from './components/LibraryPane.svelte';
  import SettingsPane from './components/SettingsPane.svelte';
  import Toast from './components/Toast.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { isSpanBody } from '$lib/types/profile';
  import { toast } from '$lib/stores/toast.svelte';

  type Tab = 'profiles' | 'library' | 'settings';
  let tab = $state<Tab>('profiles');
  let dragOverlay = $state(false);
  let flashIndices = $state<number[]>([]);
  let applyOverlay = $state(false);

  const draft = $derived(profileStore.draft);
  const span = $derived(draft && isSpanBody(draft.body) ? draft.body : null);
  const imagePath = $derived(span && span.source.type === 'single' ? span.source.path : null);
  const fit = $derived(span?.fit ?? 'fill');
  const offset = $derived<[number, number]>(span ? [span.offset[0], span.offset[1]] : [0, 0]);
  const bezels = $derived(draft?.bezels ?? { horizontal_mm: 0, vertical_mm: 0 });

  function commitOffset(next: [number, number]) {
    profileStore.patchDraft((d) => {
      if (isSpanBody(d.body)) d.body.offset = [Math.round(next[0]), Math.round(next[1])];
    });
  }

  function resetOffset() {
    profileStore.patchDraft((d) => {
      if (isSpanBody(d.body)) d.body.offset = [0, 0];
    });
  }

  function prepareDropTarget(): boolean {
    if (!profileStore.activeName) {
      if (!profileStore.draft) profileStore.newProfile();
      return true;
    }
    if (profileStore.selectedName === profileStore.activeName) return true;
    if (profileStore.dirty) {
      toast.error(
        'Drop not applied',
        'Save or revert the current edit before changing the active profile.',
      );
      return false;
    }
    profileStore.select(profileStore.activeName);
    return true;
  }

  function setSpanImage(path: string) {
    if (!prepareDropTarget()) return;
    profileStore.patchDraft((d) => {
      if (!isSpanBody(d.body)) {
        d.body = { type: 'span', source: { type: 'single', path }, fit: 'fill', offset: [0, 0] };
        return;
      }
      if (d.body.source.type === 'single') {
        d.body.source.path = path;
        d.body.offset = [0, 0];
        return;
      }
      d.body.source = { type: 'single', path };
      d.body.offset = [0, 0];
    });
    toast.success('Image set', path);
  }

  function handleImageLoadError(path: string, message: string) {
    toast.error('Could not preview image', `${path}: ${message}`);
  }

  async function applyDraft() {
    if (!draft) return;
    if (profileStore.dirty) {
      const saved = await profileStore.save();
      if (saved) triggerApply(draft.name);
      return;
    }
    triggerApply(draft.name);
  }

  function triggerApply(name: string) {
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) {
      void profileStore.apply(name);
      return;
    }
    applyOverlay = true;
    flashIndices = monitorStore.monitors.map((_, i) => i);
    window.setTimeout(() => {
      flashIndices = [];
    }, 220);
    window.setTimeout(() => {
      applyOverlay = false;
    }, 380);
    void profileStore.apply(name);
  }

  onMount(() => {
    void monitorStore.refresh();
    void profileStore.refresh();

    let unlisten: UnlistenFn | undefined;
    let unlistenDrop: UnlistenFn | undefined;
    void listen('tray://open-settings', () => {
      tab = 'settings';
    }).then((fn) => {
      unlisten = fn;
    });

    void getCurrentWebview()
      .onDragDropEvent((ev) => {
        if (ev.payload.type === 'over') {
          dragOverlay = true;
        } else if (ev.payload.type === 'leave') {
          dragOverlay = false;
        } else if (ev.payload.type === 'drop') {
          dragOverlay = false;
          const path = ev.payload.paths[0];
          if (path) setSpanImage(path);
        }
      })
      .then((fn) => {
        unlistenDrop = fn;
      });

    const interval = window.setInterval(() => {
      void profileStore.refresh();
    }, 5000);

    return () => {
      unlisten?.();
      unlistenDrop?.();
      window.clearInterval(interval);
    };
  });
</script>

<div class="flex h-full w-full">
  <Sidebar bind:tab />

  <main class="flex flex-1 flex-col gap-3 p-4">
    <section class="flex flex-1 gap-3 overflow-hidden">
      <div class="relative flex-1 min-w-0">
        <MonitorCanvas
          monitors={monitorStore.monitors}
          bezelHmm={bezels.horizontal_mm}
          bezelVmm={bezels.vertical_mm}
          {fit}
          {imagePath}
          {offset}
          onOffsetCommit={commitOffset}
          onResetOffset={resetOffset}
          onImageLoadError={handleImageLoadError}
          {flashIndices}
        />
        {#if applyOverlay}
          <div
            class="pointer-events-none absolute inset-0 rounded border border-accent/60 bg-accent/10 transition-opacity duration-300"
          ></div>
        {/if}
        {#if draft && tab === 'profiles'}
          <div class="absolute bottom-2 right-2">
            <button
              type="button"
              class="rounded bg-accent px-3 py-1.5 text-xs font-semibold text-slate-900 hover:bg-accent/90 disabled:opacity-50"
              onclick={applyDraft}
              disabled={!draft.name.trim() || profileStore.saving}
            >
              Apply
            </button>
          </div>
        {/if}
      </div>
      {#if tab === 'profiles'}
        <aside
          class="w-[320px] shrink-0 overflow-hidden rounded border border-slate-800 bg-slate-900/30 p-2"
        >
          <ProfileEditor />
        </aside>
      {/if}
    </section>

    <section
      class="h-[260px] shrink-0 overflow-auto rounded border border-slate-800 bg-slate-900/30 p-3"
    >
      {#if tab === 'profiles'}
        <ProfileList />
      {:else if tab === 'library'}
        <LibraryPane />
      {:else}
        <SettingsPane />
      {/if}
    </section>
  </main>

  {#if dragOverlay}
    <div
      class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-slate-950/60 text-sm text-slate-100"
    >
      Drop image to set as profile source…
    </div>
  {/if}
</div>

<Toast />
