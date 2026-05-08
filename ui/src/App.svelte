<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, errorMessage, type Profile } from '$lib/api';
  import { canvasView, type MonitorOverride } from '$lib/stores/canvasView.svelte';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import { applyDocumentTokens } from '$lib/stores/ui.svelte';
  import {
    buildPreviewMonitors,
    bbox,
    coverImageRect,
    defaultOverrides,
    stableId,
    totalPixels,
  } from '$lib/canvas/previewLayout';
  import { loadSourceImage, peekSourceImage } from '$lib/canvas/sourceImage';
  import {
    defaultBezels,
    isPerMonitorBody,
    isSpanBody,
    type FitMode,
    type PerMonitorAssignment,
    type ProfileV2,
  } from '$lib/types/profile';
  import PreviewCanvas from './components/canvas/PreviewCanvas.svelte';
  import BezelDock from './components/chrome/BezelDock.svelte';
  import ModeHint from './components/chrome/ModeHint.svelte';
  import MonitorInspector from './components/chrome/MonitorInspector.svelte';
  import SourceDock from './components/chrome/SourceDock.svelte';
  import TitleBar from './components/chrome/TitleBar.svelte';
  import Toast from './components/chrome/Toast.svelte';
  import ToolDock from './components/chrome/ToolDock.svelte';
  import LibraryModal from './components/overlays/LibraryModal.svelte';
  import SettingsModal from './components/overlays/SettingsModal.svelte';
  import TrayPopover from './components/overlays/TrayPopover.svelte';

  type ImageTransform = {
    offsetMmX: number;
    offsetMmY: number;
    widthMm: number;
    heightMm: number;
  };

  let libraryOpen = $state(false);
  let settingsOpen = $state(false);
  let settingsSection = $state<'general' | 'monitors'>('general');
  let trayOpen = $state(false);
  let dragOverlay = $state(false);

  let imageTransform = $state<ImageTransform>({
    offsetMmX: 0,
    offsetMmY: 0,
    widthMm: 1800,
    heightMm: 506.25,
  });
  let imageUrl = $state<string | null>(null);
  let imageNaturalDims = $state<{ w: number; h: number } | null>(null);
  let initializedTransformFor = '';

  // Slideshow runtime state — fetched from currentState() periodically.
  let slideshow = $state<{ paused: boolean; index: number; total: number } | null>(null);

  applyDocumentTokens();

  const draft = $derived<ProfileV2 | null>(profileStore.draft);
  const span = $derived(draft && isSpanBody(draft.body) ? draft.body : null);
  const fit = $derived<FitMode>(span?.fit ?? 'fill');
  const bezels = $derived(draft?.bezels ?? defaultBezels());
  const sourcePath = $derived(span && span.source.type === 'single' ? span.source.path : null);

  const previewMonitors = $derived(
    buildPreviewMonitors(monitorStore.monitors, canvasView.overrides),
  );
  const layoutMm = $derived.by(() => {
    const bb = bbox(previewMonitors);
    return { w: bb.w, h: bb.h };
  });
  const totalPx = $derived(totalPixels(previewMonitors));

  const selectedMonitor = $derived(
    canvasView.selectId
      ? (previewMonitors.find((m) => m.id === canvasView.selectId) ?? null)
      : null,
  );

  const canApply = $derived(Boolean(draft && draft.name.trim() && !profileStore.saving));

  // Initialise (or re-initialise) preview overrides whenever the detected
  // monitor list changes shape — preserves user overrides on incremental
  // updates by only overwriting unknown ids.
  $effect(() => {
    const detected = monitorStore.monitors;
    const hMm = bezels.horizontal_mm;
    if (detected.length === 0) return;
    untrack(() => {
      const defaults = defaultOverrides(detected, hMm);
      const current = canvasView.overrides;
      const next: Record<string, MonitorOverride> = { ...defaults };
      for (const id of Object.keys(defaults)) {
        const ex = current[id];
        if (ex) next[id] = ex;
      }
      canvasView.setOverrides(next);
    });
  });

  // Load the source image when the profile points at a single file.
  $effect(() => {
    const path = sourcePath;
    if (!path) {
      imageUrl = null;
      imageNaturalDims = null;
      initializedTransformFor = '';
      return;
    }
    const cached = peekSourceImage(path);
    if (cached) {
      imageUrl = cached.url;
      imageNaturalDims = { w: cached.naturalW, h: cached.naturalH };
      initialiseTransform(path);
      return;
    }
    let cancelled = false;
    void loadSourceImage(path)
      .then((img) => {
        if (cancelled || sourcePath !== path) return;
        imageUrl = img.url;
        imageNaturalDims = { w: img.naturalW, h: img.naturalH };
        initialiseTransform(path);
      })
      .catch((err: unknown) => {
        if (cancelled || sourcePath !== path) return;
        toast.error('Could not load image', errorMessage(err));
      });
    return () => {
      cancelled = true;
    };
  });

  function initialiseTransform(path: string) {
    if (initializedTransformFor === path) return;
    if (!imageNaturalDims || previewMonitors.length === 0) return;
    const aspect = imageNaturalDims.w / imageNaturalDims.h;
    imageTransform = coverImageRect(previewMonitors, aspect);
    initializedTransformFor = path;
  }

  function snapCover() {
    if (!imageNaturalDims) {
      toast.info('No image loaded', 'pick one from the library first');
      return;
    }
    imageTransform = coverImageRect(previewMonitors, imageNaturalDims.w / imageNaturalDims.h);
    toast.info(
      'Snapped image to cover',
      `${Math.round(imageTransform.widthMm)}×${Math.round(imageTransform.heightMm)} mm`,
    );
  }

  function resetTransform() {
    if (!imageNaturalDims) return;
    imageTransform = coverImageRect(previewMonitors, imageNaturalDims.w / imageNaturalDims.h);
    toast.info('Image transform reset');
  }

  function resetLayout() {
    canvasView.resetOverrides(defaultOverrides(monitorStore.monitors, bezels.horizontal_mm));
    toast.info('Monitor layout reset');
  }

  function setBezels(h: number, v: number) {
    profileStore.patchDraft((d) => {
      d.bezels = { horizontal_mm: h, vertical_mm: v };
    });
  }

  function setFit(f: FitMode) {
    profileStore.patchDraft((d) => {
      if (!isSpanBody(d.body)) return;
      d.body.fit = f;
    });
  }

  function setSpanImage(path: string) {
    if (!profileStore.draft) profileStore.newProfile();
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
      } else {
        d.body.source = { type: 'single', path };
      }
    });
    toast.success('Source updated', path.split('/').pop() ?? path);
  }

  function pinImageToMonitor(monitorId: string, path: string) {
    if (!profileStore.draft) profileStore.newProfile();
    const detected = monitorStore.monitors.find((m) => stableId(m) === monitorId);
    if (!detected) {
      toast.error('Drop ignored', 'monitor not found in layout');
      return;
    }
    const assignment: PerMonitorAssignment = {
      monitor: { stable_id: detected.stable_id ?? '', name: detected.name },
      path,
    };
    profileStore.patchDraft((d) => {
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
    toast.success('Image pinned', `${detected.name}: ${path.split('/').pop() ?? path}`);
  }

  async function refreshRuntime() {
    try {
      const s = await api.currentState();
      if (s.slideshow) {
        slideshow = {
          paused: s.slideshow.paused,
          index: s.slideshow.current_index ?? 0,
          total: s.slideshow.history_len + 1,
        };
      } else {
        slideshow = null;
      }
    } catch {
      // ignore — `currentState` may not be reachable yet at startup
    }
  }

  async function applyDraft() {
    if (!draft) return;
    if (profileStore.dirty) {
      const saved = await profileStore.save();
      if (!saved) return;
    }
    try {
      const t0 = performance.now();
      const r = await api.applyProfile(draft.name);
      const elapsed = r.elapsed_ms ?? Math.round(performance.now() - t0);
      runtime.recordApply({
        backend: r.backend ?? 'unknown',
        elapsedMs: elapsed,
        monitorsSet: r.monitors_set ?? monitorStore.monitors.length,
        at: Date.now(),
      });
      toast.success(`Applied '${draft.name}'`, `${r.backend ?? 'backend'} · ${elapsed} ms`);
      void profileStore.refresh();
    } catch (err) {
      toast.error('Apply failed', errorMessage(err));
    }
  }

  async function switchProfile(p: Profile) {
    profileStore.select(p.name);
    try {
      const t0 = performance.now();
      const r = await api.applyProfile(p.name);
      const elapsed = r.elapsed_ms ?? Math.round(performance.now() - t0);
      runtime.recordApply({
        backend: r.backend ?? 'unknown',
        elapsedMs: elapsed,
        monitorsSet: r.monitors_set ?? monitorStore.monitors.length,
        at: Date.now(),
      });
      toast.success(`Switched to ${p.name}`);
      void profileStore.refresh();
    } catch (err) {
      toast.error(`Failed to apply '${p.name}'`, errorMessage(err));
    }
  }

  async function slideshowNext() {
    try {
      await api.slideshowNext();
      await refreshRuntime();
    } catch (err) {
      toast.error('Slideshow next failed', errorMessage(err));
    }
  }

  async function slideshowPrev() {
    try {
      await api.slideshowPrev();
      await refreshRuntime();
    } catch (err) {
      toast.error('Slideshow prev failed', errorMessage(err));
    }
  }

  async function slideshowTogglePause() {
    try {
      const r = await api.slideshowPause();
      slideshow = slideshow ? { ...slideshow, paused: r.paused } : null;
    } catch (err) {
      toast.error('Slideshow pause failed', errorMessage(err));
    }
  }

  function onKey(e: KeyboardEvent) {
    const isInput = ['INPUT', 'TEXTAREA', 'SELECT'].includes(
      (e.target as HTMLElement)?.tagName ?? '',
    );
    if (e.key === 'Escape') {
      if (settingsOpen) settingsOpen = false;
      else if (libraryOpen) libraryOpen = false;
      else if (trayOpen) trayOpen = false;
      else if (canvasView.selectId) canvasView.setSelectId(null);
      return;
    }
    if (isInput) return;
    if (e.key === 'Enter' && !libraryOpen && !settingsOpen) {
      if (canApply) void applyDraft();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === ',') {
      e.preventDefault();
      settingsOpen = true;
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'l') {
      e.preventDefault();
      libraryOpen = true;
      return;
    }
    if ((e.metaKey || e.ctrlKey) && /^[123]$/.test(e.key)) {
      e.preventDefault();
      const p = profileStore.profiles[Number.parseInt(e.key, 10) - 1];
      if (p) void switchProfile(p);
      return;
    }
    if (e.key === ' ') {
      e.preventDefault();
      void slideshowTogglePause();
      return;
    }
    if (e.key === 'ArrowRight' && !libraryOpen && !settingsOpen) void slideshowNext();
    if (e.key === 'ArrowLeft' && !libraryOpen && !settingsOpen) void slideshowPrev();
    if (e.key.toLowerCase() === 'r' && !libraryOpen && !settingsOpen) resetTransform();
    if (e.key.toLowerCase() === 'd' && !libraryOpen && !settingsOpen) canvasView.toggleDim();
    if (e.key === 'F5') {
      e.preventDefault();
      void monitorStore.refresh().then(() => {
        toast.info(`Re-detected ${monitorStore.monitors.length} monitors`);
      });
    }
    if (canvasView.selectId && (e.key === '[' || e.key === ']')) {
      const delta = e.key === ']' ? 90 : -90;
      const cur = canvasView.overrides[canvasView.selectId];
      if (cur) {
        const nextRot = (((cur.rotation + delta) % 360) + 360) % 360;
        canvasView.override(canvasView.selectId, {
          rotation: nextRot as 0 | 90 | 180 | 270,
        });
      }
    }
    if (canvasView.selectId && /^Arrow/.test(e.key)) {
      const step = e.shiftKey ? 10 : 1;
      const map: Record<string, [number, number]> = {
        ArrowUp: [0, -step],
        ArrowDown: [0, step],
        ArrowLeft: [-step, 0],
        ArrowRight: [step, 0],
      };
      const d = map[e.key];
      if (d) {
        e.preventDefault();
        const cur = canvasView.overrides[canvasView.selectId];
        if (cur)
          canvasView.override(canvasView.selectId, { xMm: cur.xMm + d[0], yMm: cur.yMm + d[1] });
      }
    }
  }

  onMount(() => {
    void monitorStore.refresh();
    void profileStore.refresh().then(refreshRuntime);
    void libraryStore.refresh();

    let unTray: UnlistenFn | undefined;
    let unDrop: UnlistenFn | undefined;

    void listen('tray://open-settings', () => {
      settingsOpen = true;
    }).then((fn) => {
      unTray = fn;
    });

    void getCurrentWebview()
      .onDragDropEvent((ev) => {
        if (ev.payload.type === 'over') dragOverlay = true;
        else if (ev.payload.type === 'leave') dragOverlay = false;
        else if (ev.payload.type === 'drop') {
          dragOverlay = false;
          const path = ev.payload.paths[0];
          if (path) setSpanImage(path);
        }
      })
      .then((fn) => {
        unDrop = fn;
      });

    window.addEventListener('keydown', onKey);
    const interval = window.setInterval(() => {
      void profileStore.refresh();
      void refreshRuntime();
    }, 5000);

    return () => {
      unTray?.();
      unDrop?.();
      window.clearInterval(interval);
      window.removeEventListener('keydown', onKey);
    };
  });

  async function openMainWindow() {
    try {
      const w = getCurrentWindow();
      await w.show();
      await w.unminimize();
      await w.setFocus();
    } catch {
      // ignore
    }
  }

  async function quitApp() {
    try {
      await getCurrentWindow().destroy();
    } catch {
      // ignore
    }
  }

  function setPrimary() {
    const id = canvasView.selectId;
    if (!id) return;
    toast.info('Primary change is preview-only', 'will not be pushed to compositor');
  }

  function rotateSelected(delta: number) {
    const id = canvasView.selectId;
    if (!id) return;
    const cur = canvasView.overrides[id];
    if (!cur) return;
    const nextRot = (((cur.rotation + delta) % 360) + 360) % 360;
    canvasView.override(id, { rotation: nextRot as 0 | 90 | 180 | 270 });
  }

  // Source dock metadata — lifted from the active draft for display.
  const sourceName = $derived(
    sourcePath ? (sourcePath.split('/').pop() ?? sourcePath) : '— no source',
  );
  const sourceMeta = $derived(
    imageNaturalDims
      ? `${imageNaturalDims.w}×${imageNaturalDims.h}`
      : sourcePath
        ? 'loading…'
        : 'pick from library',
  );
  const backendName = $derived(runtime.last?.backend ?? 'auto-detect');

  const someMissingMm = $derived(
    monitorStore.monitors.length > 0 && monitorStore.monitors.some((m) => !m.physical_size_mm),
  );
</script>

<div class="fixed inset-0 overflow-hidden">
  <PreviewCanvas
    monitors={monitorStore.monitors}
    bezelHmm={bezels.horizontal_mm}
    {imageUrl}
    {imageTransform}
    onImageTransformChange={(t) => (imageTransform = t)}
    onMonitorDrop={pinImageToMonitor}
  />

  <ModeHint />

  <TitleBar
    profiles={profileStore.profiles}
    activeName={profileStore.activeName}
    {backendName}
    {canApply}
    onApply={() => void applyDraft()}
    onSwitchProfile={(p) => void switchProfile(p)}
    onNewProfile={() => profileStore.newProfile()}
    onOpenLibrary={() => (libraryOpen = true)}
    onOpenSettings={() => (settingsOpen = true)}
    onTrayClick={() => (trayOpen = !trayOpen)}
  />

  <ToolDock onResetTransform={resetTransform} onSnapCover={snapCover} onResetLayout={resetLayout} />

  <BezelDock
    bezelHmm={bezels.horizontal_mm}
    bezelVmm={bezels.vertical_mm}
    onBezelChange={setBezels}
    fitMode={fit}
    onFitChange={setFit}
    {layoutMm}
    monitorCount={monitorStore.monitors.length}
    {totalPx}
  />

  <SourceDock
    {sourceName}
    {sourceMeta}
    sourceThumbUrl={imageUrl}
    {slideshow}
    onPrev={() => void slideshowPrev()}
    onNext={() => void slideshowNext()}
    onTogglePause={() => void slideshowTogglePause()}
    onOpenLibrary={() => (libraryOpen = true)}
  />

  {#if selectedMonitor}
    <MonitorInspector
      monitor={selectedMonitor}
      {imageUrl}
      {imageTransform}
      onClose={() => canvasView.setSelectId(null)}
      onSetPrimary={setPrimary}
      onRotate={rotateSelected}
    />
  {/if}

  {#if someMissingMm}
    <div class="banner">
      <span class="dot warn"></span>
      <span>One or more monitors are missing physical size — bezel math will be approximate.</span>
      <button
        class="btn sm"
        onclick={() => {
          settingsSection = 'monitors';
          settingsOpen = true;
        }}>Fix</button
      >
    </div>
  {/if}

  {#if libraryOpen}
    <LibraryModal
      onClose={() => (libraryOpen = false)}
      onApplyAsSpan={setSpanImage}
      onPinToMonitor={pinImageToMonitor}
    />
  {/if}

  {#if settingsOpen}
    <SettingsModal
      initialSection={settingsSection}
      onClose={() => {
        settingsOpen = false;
        settingsSection = 'general';
      }}
    />
  {/if}

  {#if trayOpen}
    <TrayPopover
      profiles={profileStore.profiles}
      activeName={profileStore.activeName}
      slideshowPaused={slideshow?.paused ?? false}
      onSwitch={(p) => {
        trayOpen = false;
        void switchProfile(p);
      }}
      onPrev={() => void slideshowPrev()}
      onNext={() => void slideshowNext()}
      onTogglePause={() => void slideshowTogglePause()}
      onOpenSettings={() => {
        trayOpen = false;
        settingsOpen = true;
      }}
      onOpenWindow={() => {
        trayOpen = false;
        void openMainWindow();
      }}
      onQuit={() => {
        trayOpen = false;
        void quitApp();
      }}
      onClose={() => (trayOpen = false)}
    />
  {/if}

  {#if dragOverlay}
    <div class="dnd-overlay">Drop image to set as profile source…</div>
  {/if}

  <Toast />
</div>

<style>
  .banner {
    position: absolute;
    left: 50%;
    top: 56px;
    transform: translateX(-50%);
    background: color-mix(in oklab, var(--warn) 18%, var(--panel));
    border: 1px solid color-mix(in oklab, var(--warn) 50%, var(--line));
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 12px;
    z-index: 4;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .dnd-overlay {
    position: absolute;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: oklch(0 0 0 / 0.6);
    color: var(--text);
    font-size: 14px;
    pointer-events: none;
  }
</style>
