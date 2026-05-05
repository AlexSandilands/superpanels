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
  import {
    defaultBezels,
    isPerMonitorBody,
    isSpanBody,
    type PerMonitorAssignment,
    type ProfileV2,
  } from '$lib/types/profile';
  import { toast } from '$lib/stores/toast.svelte';

  type Tab = 'profiles' | 'library' | 'settings';
  let tab = $state<Tab>('profiles');
  let dragOverlay = $state(false);
  let flashIndices = $state<number[]>([]);
  let applyOverlay = $state(false);

  const PANEL_KEY = 'superpanels.bottomPanelHeight';
  const PANEL_MIN = 120;
  const PANEL_MAX_FRACTION = 0.85;
  let panelHeight = $state<number>(readPanelHeight());
  let resizing = $state(false);
  let resizeStartY = 0;
  let resizeStartH = 0;

  function readPanelHeight(): number {
    if (typeof window === 'undefined') return 260;
    const raw = window.localStorage?.getItem(PANEL_KEY);
    const n = raw ? Number.parseInt(raw, 10) : NaN;
    return Number.isFinite(n) && n >= PANEL_MIN ? n : 260;
  }

  function writePanelHeight(h: number) {
    try {
      window.localStorage?.setItem(PANEL_KEY, String(Math.round(h)));
    } catch {
      // localStorage may be unavailable (private mode); ignore.
    }
  }

  function startResize(ev: PointerEvent) {
    ev.preventDefault();
    resizing = true;
    resizeStartY = ev.clientY;
    resizeStartH = panelHeight;
    (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
  }

  function moveResize(ev: PointerEvent) {
    if (!resizing) return;
    const max = Math.max(PANEL_MIN, window.innerHeight * PANEL_MAX_FRACTION);
    const next = Math.max(PANEL_MIN, Math.min(max, resizeStartH - (ev.clientY - resizeStartY)));
    panelHeight = next;
  }

  function endResize(ev: PointerEvent) {
    if (!resizing) return;
    resizing = false;
    (ev.currentTarget as HTMLElement).releasePointerCapture(ev.pointerId);
    writePanelHeight(panelHeight);
  }

  function onResizeKey(ev: KeyboardEvent) {
    const step = ev.shiftKey ? 80 : 24;
    if (ev.key === 'ArrowUp') {
      ev.preventDefault();
      panelHeight = Math.min(window.innerHeight * PANEL_MAX_FRACTION, panelHeight + step);
      writePanelHeight(panelHeight);
    } else if (ev.key === 'ArrowDown') {
      ev.preventDefault();
      panelHeight = Math.max(PANEL_MIN, panelHeight - step);
      writePanelHeight(panelHeight);
    }
  }

  const draft = $derived(profileStore.draft);
  const span = $derived(draft && isSpanBody(draft.body) ? draft.body : null);
  const imagePath = $derived(span && span.source.type === 'single' ? span.source.path : null);
  const fit = $derived(span?.fit ?? 'fill');
  const offset = $derived<[number, number]>(span ? [span.offset[0], span.offset[1]] : [0, 0]);
  const imageSizePx = $derived<[number, number] | null>(
    span?.image_size_px ? [span.image_size_px[0], span.image_size_px[1]] : null,
  );
  const bezels = $derived(draft?.bezels ?? { horizontal_mm: 0, vertical_mm: 0 });

  function commitTransform(next: [number, number], nextSize: [number, number] | null) {
    profileStore.patchDraft((d) => {
      if (!isSpanBody(d.body)) return;
      d.body.offset = [Math.round(next[0]), Math.round(next[1])];
      if (nextSize) {
        d.body.image_size_px = [
          Math.max(1, Math.round(nextSize[0])),
          Math.max(1, Math.round(nextSize[1])),
        ];
      } else {
        d.body.image_size_px = null;
      }
    });
  }

  function resetTransform() {
    profileStore.patchDraft((d) => {
      if (!isSpanBody(d.body)) return;
      d.body.offset = [0, 0];
      d.body.image_size_px = null;
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

  function pinImageToMonitor(monitorIndex: number, path: string) {
    if (!prepareDropTarget()) return;
    const m = monitorStore.monitors[monitorIndex];
    if (!m) {
      toast.error('Drop ignored', 'monitor not found in layout');
      return;
    }
    const assignment: PerMonitorAssignment = {
      monitor: { stable_id: m.stable_id ?? '', name: m.name },
      path,
    };
    profileStore.patchDraft((d: ProfileV2) => {
      if (!isPerMonitorBody(d.body)) {
        d.body = { type: 'per_monitor', assignments: [assignment], fit: 'fill' };
        return;
      }
      const idx = d.body.assignments.findIndex(
        (a) =>
          (a.monitor.stable_id !== '' && a.monitor.stable_id === assignment.monitor.stable_id) ||
          a.monitor.name === assignment.monitor.name,
      );
      if (idx >= 0) d.body.assignments[idx] = assignment;
      else d.body.assignments.push(assignment);
    });
    if (!profileStore.draft) {
      const base: ProfileV2 = {
        name: 'untitled',
        body: { type: 'per_monitor', assignments: [assignment], fit: 'fill' },
        bezels: defaultBezels(),
      };
      profileStore.replaceDraft(base);
    }
    toast.success('Image pinned', `${m.name}: ${path.split('/').pop() ?? path}`);
  }

  function setSpanImage(path: string) {
    if (!prepareDropTarget()) return;
    profileStore.patchDraft((d) => {
      if (!isSpanBody(d.body)) {
        d.body = {
          type: 'span',
          source: { type: 'single', path },
          fit: 'fill',
          offset: [0, 0],
          image_size_px: null,
        };
        return;
      }
      if (d.body.source.type === 'single') {
        d.body.source.path = path;
        d.body.offset = [0, 0];
        d.body.image_size_px = null;
        return;
      }
      d.body.source = { type: 'single', path };
      d.body.offset = [0, 0];
      d.body.image_size_px = null;
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
          {imageSizePx}
          onTransformCommit={commitTransform}
          onResetTransform={resetTransform}
          onMonitorDrop={pinImageToMonitor}
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

    <!-- reason: a horizontal resize gripper is a real interactive control,
         but `role="separator"` is the correct ARIA role per WAI-ARIA's
         resize pattern; svelte-eslint flags `tabindex` on a non-interactive
         element by default. The keyboard handler is the canonical a11y
         affordance for this pattern, so the warnings are intentional. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      tabindex="0"
      aria-orientation="horizontal"
      aria-label="Resize bottom panel"
      class="group relative -my-1.5 flex h-3 shrink-0 cursor-row-resize items-center justify-center"
      class:bg-accent={resizing}
      class:bg-opacity-20={resizing}
      onpointerdown={startResize}
      onpointermove={moveResize}
      onpointerup={endResize}
      onpointercancel={endResize}
      onkeydown={onResizeKey}
    >
      <span class="h-1 w-12 rounded bg-slate-700 group-hover:bg-accent" class:bg-accent={resizing}
      ></span>
    </div>

    <section
      class="shrink-0 overflow-auto rounded border border-slate-800 bg-slate-900/30 p-3"
      style:height={`${panelHeight}px`}
    >
      {#if tab === 'profiles'}
        <ProfileList />
      {:else if tab === 'library'}
        <LibraryPane onApplyAsSpan={setSpanImage} />
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
