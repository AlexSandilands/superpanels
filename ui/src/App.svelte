<script lang="ts">
  import { onMount } from 'svelte';
  import type { Profile } from '$lib/api';
  import { canvasView } from '$lib/stores/canvasView.svelte';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import { applyDocumentTokens } from '$lib/stores/ui.svelte';
  import {
    imageTransform,
    seedOverridesFromMonitors,
    sourceImageState,
    useSourceImage,
  } from '$lib/stores/imageTransform.svelte';
  import {
    applyDraftProfile,
    openMainWindow,
    pinImageToMonitor,
    quitApp,
    setSpanImage,
    switchAndApply,
  } from '$lib/actions';
  import {
    buildPreviewMonitors,
    bbox,
    hNeighbourPairs,
    totalPixels,
    uniformGap,
    vNeighbourPairs,
  } from '$lib/canvas/previewLayout';
  import { rotateSelected } from '$lib/canvas/select';
  import {
    resetLayout as runResetLayout,
    resetTransform as runResetTransform,
    setHGap as runSetHGap,
    setVGap as runSetVGap,
    snapCover as runSnapCover,
  } from '$lib/canvas/transformActions';
  import { slideshowController } from '$lib/slideshowController.svelte';
  import { attachWindowEvents } from '$lib/events/window';
  import { dispatchKey } from '$lib/keymap';
  import { defaultBezels, isSpanBody } from '$lib/types/profile';
  import PreviewCanvas from './components/canvas/PreviewCanvas.svelte';
  import BezelDock from './components/chrome/BezelDock.svelte';
  import ModeHint from './components/chrome/ModeHint.svelte';
  import MonitorInspector from './components/chrome/MonitorInspector.svelte';
  import SourceDock from './components/chrome/SourceDock.svelte';
  import TitleBar from './components/chrome/TitleBar.svelte';
  import Toast from './components/widgets/Toast.svelte';
  import ToolDock from './components/chrome/ToolDock.svelte';
  import LibraryModal from './components/overlays/LibraryModal.svelte';
  import SettingsModal from './components/overlays/SettingsModal.svelte';
  import TrayPopover from './components/overlays/TrayPopover.svelte';

  let libraryOpen = $state(false);
  let settingsOpen = $state(false);
  let settingsSection = $state<'general' | 'monitors'>('general');
  let trayOpen = $state(false);
  let dragOverlay = $state(false);

  applyDocumentTokens();

  const draft = $derived<Profile | null>(profileStore.draft);
  const span = $derived(draft && isSpanBody(draft.body) ? draft.body : null);
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
  const hPairs = $derived(hNeighbourPairs(previewMonitors));
  const vPairs = $derived(vNeighbourPairs(previewMonitors));
  const currentHGap = $derived(uniformGap(hPairs));
  const currentVGap = $derived(uniformGap(vPairs));
  const hMixed = $derived(hPairs.length >= 2 && currentHGap === null);
  const vMixed = $derived(vPairs.length >= 2 && currentVGap === null);

  const selectedMonitor = $derived(
    canvasView.selectId
      ? (previewMonitors.find((m) => m.id === canvasView.selectId) ?? null)
      : null,
  );

  const canApply = $derived(Boolean(draft && draft.name.trim() && !profileStore.saving));

  seedOverridesFromMonitors(
    () => monitorStore.monitors,
    () => bezels.horizontal_mm,
  );

  useSourceImage(
    () => sourcePath,
    () => previewMonitors,
  );

  const imageUrl = $derived(sourceImageState.value.url);
  const imageNaturalDims = $derived(sourceImageState.value.naturalDims);

  const snapCover = () => runSnapCover(previewMonitors, imageNaturalDims);
  const resetTransform = () => runResetTransform(previewMonitors, imageNaturalDims);
  const resetLayout = () => runResetLayout(bezels.horizontal_mm);
  const setHGap = (h: number) => runSetHGap(previewMonitors, bezels.vertical_mm, h);
  const setVGap = (v: number) => runSetVGap(previewMonitors, bezels.horizontal_mm, v);

  function onKey(e: KeyboardEvent) {
    dispatchKey(e, {
      overlays: {
        libraryOpen,
        settingsOpen,
        trayOpen,
        setLibraryOpen: (v) => (libraryOpen = v),
        setSettingsOpen: (v) => (settingsOpen = v),
        setTrayOpen: (v) => (trayOpen = v),
      },
      canApply,
      resetTransform,
    });
  }

  onMount(() => {
    void monitorStore.refresh();
    void profileStore.refresh().then(() => slideshowController.refresh());
    void libraryStore.refresh();

    const detachWindow = attachWindowEvents({
      onOpenSettings: () => (settingsOpen = true),
      onDragOver: () => (dragOverlay = true),
      onDragLeave: () => (dragOverlay = false),
      onDrop: (path) => setSpanImage(path),
    });

    window.addEventListener('keydown', onKey);
    const interval = window.setInterval(() => {
      void profileStore.refresh();
      void slideshowController.refresh();
    }, 5000);

    return () => {
      detachWindow();
      window.clearInterval(interval);
      window.removeEventListener('keydown', onKey);
    };
  });

  function setPrimary() {
    if (!canvasView.selectId) return;
    toast.info('Primary change is preview-only', 'will not be pushed to compositor');
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
    imageTransform={imageTransform.value}
    onImageTransformChange={(t) => imageTransform.set(t)}
    onMonitorDrop={pinImageToMonitor}
  />

  <ModeHint />

  <TitleBar
    profiles={profileStore.profiles}
    activeName={profileStore.activeName}
    {backendName}
    {canApply}
    onApply={() => void applyDraftProfile()}
    onSwitchProfile={(p) => void switchAndApply(p)}
    onNewProfile={() => profileStore.newProfile()}
    onOpenLibrary={() => (libraryOpen = true)}
    onOpenSettings={() => (settingsOpen = true)}
    onTrayClick={() => (trayOpen = !trayOpen)}
  />

  <ToolDock onResetTransform={resetTransform} onSnapCover={snapCover} onResetLayout={resetLayout} />

  <BezelDock
    hGapMm={currentHGap}
    vGapMm={currentVGap}
    {hMixed}
    {vMixed}
    fallbackHmm={bezels.horizontal_mm}
    fallbackVmm={bezels.vertical_mm}
    onHGapChange={setHGap}
    onVGapChange={setVGap}
    {layoutMm}
    monitorCount={monitorStore.monitors.length}
    {totalPx}
  />

  <SourceDock
    {sourceName}
    {sourceMeta}
    sourceThumbUrl={imageUrl}
    slideshow={slideshowController.state}
    onPrev={() => void slideshowController.prev()}
    onNext={() => void slideshowController.next()}
    onTogglePause={() => void slideshowController.togglePause()}
    onOpenLibrary={() => (libraryOpen = true)}
  />

  {#if selectedMonitor}
    <MonitorInspector
      monitor={selectedMonitor}
      {imageUrl}
      imageTransform={imageTransform.value}
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
      slideshowPaused={slideshowController.state?.paused ?? false}
      onSwitch={(p) => {
        trayOpen = false;
        void switchAndApply(p);
      }}
      onPrev={() => void slideshowController.prev()}
      onNext={() => void slideshowController.next()}
      onTogglePause={() => void slideshowController.togglePause()}
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
