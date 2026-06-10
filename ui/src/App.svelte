<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { Profile } from '$lib/api';
  import { canvasView } from '$lib/stores/canvas-view.svelte';
  import { daemonStatus } from '$lib/stores/daemon-status.svelte';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { preemption } from '$lib/stores/preemption.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import { applyDocumentTokens } from '$lib/stores/ui.svelte';
  import {
    imageTransform,
    seedOverridesFromMonitors,
    sourceImageState,
    useSourceImage,
  } from '$lib/stores/image-transform.svelte';
  import {
    applyDraftProfile,
    applyMonitorStateToCanvas,
    openMainWindow,
    persistSlideshowSource,
    pinImageToMonitor,
    quitApp,
    revertCanvasToActive,
    saveActiveProfile,
    setSpanImage,
    switchAndApply,
  } from '$lib/actions';
  import { canvasOverridesDirty, imageTransformDirty } from '$lib/canvas/dirty';
  import { buildPreviewMonitors } from '$lib/canvas/preview-layout';
  import {
    resetLayout as runResetLayout,
    resetTransform as runResetTransform,
    setHGap as runSetHGap,
    setVGap as runSetVGap,
    snapCover as runSnapCover,
  } from '$lib/canvas/transform-actions';
  import {
    bbox,
    hNeighbourPairs,
    totalPixels,
    uniformGap,
    vNeighbourPairs,
  } from '$lib/canvas/preview-layout';
  import { slideshowController } from '$lib/slideshow-controller.svelte';
  import { attachWindowEvents } from '$lib/events/window';
  import { dispatchKey } from '$lib/keymap';
  import {
    defaultSlideshowConfig,
    isSlideshowSource,
    isSpanBody,
    type ImageSet,
    type ProfileKind,
    type SlideshowConfig,
    type SlideshowSource,
  } from '$lib/types/profile-helpers';
  import PreviewCanvas from './components/canvas/PreviewCanvas.svelte';
  import ModeHint from './components/chrome/ModeHint.svelte';
  import MonitorGapDock from './components/chrome/MonitorGapDock.svelte';
  import MonitorInspector from './components/chrome/MonitorInspector.svelte';
  import SourceDock from './components/chrome/SourceDock.svelte';
  import TitleBar from './components/chrome/TitleBar.svelte';
  import Toast from './components/widgets/Toast.svelte';
  import ToolDock from './components/chrome/ToolDock.svelte';
  import ConfirmDiscardModal from './components/overlays/ConfirmDiscardModal.svelte';
  import LibraryModal from './components/overlays/LibraryModal.svelte';
  import ProfileManagerModal from './components/overlays/ProfileManagerModal.svelte';
  import SettingsModal from './components/overlays/SettingsModal.svelte';
  import SaveProfileDialog from './components/overlays/SaveProfileDialog.svelte';
  import TrayPopover from './components/overlays/TrayPopover.svelte';
  import { api, errorMessage } from '$lib/api';

  let libraryOpen = $state(false);
  let settingsOpen = $state(false);
  let settingsSection = $state<'general' | 'monitors'>('general');
  let trayOpen = $state(false);
  let dragOverlay = $state(false);
  let saveDialogOpen = $state(false);
  let profileManagerOpen = $state(false);

  async function saveAsNew(name: string, description: string | null, kind: ProfileKind = 'single') {
    saveDialogOpen = false;
    try {
      const monitor_state: Record<string, { x_mm: number; y_mm: number }> = {};
      for (const m of previewMonitors) {
        monitor_state[m.id] = {
          x_mm: m.xMm,
          y_mm: m.yMm,
        };
      }
      const t = imageTransform.value;
      const image_rect_mm = {
        x_mm: t.offsetMmX,
        y_mm: t.offsetMmY,
        w_mm: t.widthMm,
        h_mm: t.heightMm,
      };
      // Duplicating a slideshow keeps its sources and timer config. A fresh
      // slideshow starts from the current image (if any) so the desktop
      // doesn't go blank; more sources come from the library.
      const seed: ImageSet = {
        sources: sourcePath ? [{ type: 'image', path: sourcePath }] : [],
      };
      const slideshowSrc: SlideshowSource = slideshowSource
        ? ($state.snapshot(slideshowSource) as SlideshowSource)
        : { type: 'slideshow', images: seed, config: defaultSlideshowConfig() };
      const body =
        kind === 'slideshow'
          ? { type: 'span' as const, source: slideshowSrc, image_rect_mm }
          : span
            ? { ...span, image_rect_mm }
            : {
                type: 'span' as const,
                source: { type: 'single' as const, path: sourcePath ?? '' },
                image_rect_mm,
              };
      const now = new Date().toISOString();
      const profile = {
        name,
        body,
        monitor_state,
        // Topology is recomputed by the daemon against live monitors.
        topology: '',
        description,
        created_at: now,
        updated_at: now,
        last_applied_at: null,
        backend_override: null,
      };
      await api.saveProfile(profile, { recomputeTopology: true });
      await profileStore.refresh();
      toast.success(`Saved '${name}'`);
      const saved = profileStore.profiles.find((p) => p.name === name);
      if (kind === 'slideshow') {
        // A new slideshow needs populating — drop straight into the library.
        // With images on board (a duplicate, or the seeded canvas image) the
        // profile is applied first so the desktop follows along.
        const isDuplicate = slideshowSource !== null;
        const hasImages = slideshowSrc.images.sources.length > 0;
        if (saved && hasImages) {
          await switchAndApply(saved);
        } else {
          profileStore.select(name);
          toast.info('Add images', 'pick images or folders for the slideshow');
        }
        if (!isDuplicate || !hasImages) libraryOpen = true;
      } else if (saved) {
        await switchAndApply(saved);
      }
    } catch (err) {
      toast.error('Save as new failed', errorMessage(err));
    }
  }

  applyDocumentTokens();

  const draft = $derived<Profile | null>(profileStore.draft);
  const span = $derived(draft && isSpanBody(draft.body) ? draft.body : null);
  const sourcePath = $derived(span && span.source.type === 'single' ? span.source.path : null);
  const slideshowSource = $derived(span && isSlideshowSource(span.source) ? span.source : null);

  // Saved slideshow profile whose image set the library can edit. Unsaved
  // drafts are excluded — `update_profile_source` needs an on-disk profile.
  const slideshowTarget = $derived.by(() => {
    if (!draft || !slideshowSource) return null;
    if (!profileStore.profiles.some((p) => p.name === draft.name)) return null;
    return { name: draft.name, images: slideshowSource.images };
  });

  // While the active profile is a slideshow, mirror its live image onto the
  // canvas / dock thumb so the preview tracks the desktop.
  const liveSlideshowPath = $derived(
    slideshowSource && draft && draft.name === profileStore.activeName
      ? (slideshowController.state?.currentPath ?? null)
      : null,
  );

  // The dock's slideshow controls follow the profile the dock displays (the
  // draft), never whatever happens to be active — editing B while A runs
  // must not silently rewrite A's config.
  const dockSlideshowProfile = $derived(
    slideshowTarget && slideshowSource
      ? { name: slideshowTarget.name, source: slideshowSource }
      : null,
  );

  // Playback state belongs to the active slideshow; hide it while the dock
  // shows a different (merely selected) profile.
  const dockSlideshowState = $derived(
    draft && draft.name === profileStore.activeName ? slideshowController.state : null,
  );

  async function updateSlideshowConfig(config: SlideshowConfig) {
    const target = dockSlideshowProfile;
    if (!target) return;
    const ok = await persistSlideshowSource(target.name, { ...target.source, config });
    if (ok) void slideshowController.refresh();
  }

  async function updateSlideshowImages(images: ImageSet) {
    const target = slideshowTarget;
    if (!target || !slideshowSource) return;
    const ok = await persistSlideshowSource(target.name, { ...slideshowSource, images });
    if (ok) void slideshowController.refresh();
  }

  // An empty slideshow can't be applied (its pool is empty and validity
  // disables it) — switching to one selects it and opens the library to
  // populate the set instead of surfacing an apply failure.
  function pickProfile(p: Profile) {
    const emptySlideshow =
      isSpanBody(p.body) &&
      isSlideshowSource(p.body.source) &&
      p.body.source.images.sources.length === 0;
    if (emptySlideshow) {
      profileStore.select(p.name);
      applyMonitorStateToCanvas(p);
      toast.info('Slideshow has no images yet', 'pick images or folders from the library');
      libraryOpen = true;
      return;
    }
    void switchAndApply(p);
  }

  const previewMonitors = $derived(
    buildPreviewMonitors(monitorStore.monitors, canvasView.overrides),
  );

  const hPairs = $derived(hNeighbourPairs(previewMonitors));
  const vPairs = $derived(vNeighbourPairs(previewMonitors));
  const currentHGap = $derived(uniformGap(hPairs));
  const currentVGap = $derived(uniformGap(vPairs));
  const hMixed = $derived(hPairs.length >= 2 && currentHGap === null);
  const vMixed = $derived(vPairs.length >= 2 && currentVGap === null);
  const layoutMm = $derived.by(() => {
    const bb = bbox(previewMonitors);
    return { w: bb.w, h: bb.h };
  });
  const totalPx = $derived(totalPixels(previewMonitors));
  const snapHmm = $derived(currentHGap ?? 0);

  const setHGap = (h: number) => runSetHGap(previewMonitors, currentVGap ?? 0, h);
  const setVGap = (v: number) => runSetVGap(previewMonitors, currentHGap ?? 0, v);

  const selectedMonitor = $derived(
    canvasView.selectId
      ? (previewMonitors.find((m) => m.id === canvasView.selectId) ?? null)
      : null,
  );

  const canApply = $derived(Boolean(draft && draft.name.trim() && !profileStore.saving));

  // Dirty detection (§4e.11.3). The store's own `dirty` flag covers
  // explicit `patchDraft` calls (image source, body shape); on top of that we
  // diff the live canvas state — monitor overrides + image transform — against
  // the active profile so drag / rotate / image-resize edits show up here too.
  const canvasDirty = $derived.by(() => {
    if (profileStore.dirty) return true;
    const active = profileStore.activeProfile;
    if (!active) return false;
    if (canvasOverridesDirty(canvasView.overrides, active)) return true;
    return imageTransformDirty(imageTransform.value, active);
  });
  const canSave = $derived(Boolean(profileStore.activeName) && !profileStore.saving);
  const canRevert = $derived(canvasDirty && Boolean(profileStore.activeName));

  // Confirm-discard modal (§4e.11.5). When the user initiates an action
  // that would silently drop unsaved canvas state, hold the action in
  // `pendingDiscard` and surface the modal. Cancel keeps the canvas;
  // Confirm runs the held action. Schedule-driven switches do not pass
  // through this gate — see the schedule preemption toast wiring below.
  let pendingDiscard = $state<{
    label: string;
    perform: () => void | Promise<void>;
  } | null>(null);

  function guardedDiscard(label: string, perform: () => void | Promise<void>): void {
    if (canvasDirty) {
      pendingDiscard = { label, perform };
      return;
    }
    void perform();
  }

  // Schedule-preemption tracking (§4e.11.6). The sentinel + dirty-canvas
  // snapshot live in `lib/stores/preemption.svelte` so user-driven actions
  // (`applyDraftProfile`, `saveActiveProfile`, `switchAndApply`) can claim
  // an upcoming switch and avoid being misread as a schedule fire.
  $effect(() => {
    const seen = profileStore.activeName;
    const sentinel = preemption.sentinel;
    if (seen === sentinel) return;
    const externalChange = sentinel !== null && seen !== sentinel;
    if (externalChange) {
      const snapshot = preemption.consumeSnapshot();
      const prev = sentinel;
      if (snapshot !== null) {
        toast.info(`Schedule switched to '${seen ?? 'unknown'}'`, {
          detail: `Unsaved changes to '${prev}' were discarded`,
          action: {
            label: 'Undo',
            onClick: () => {
              void api
                .applyCanvas(snapshot, prev)
                .then(() => {
                  toast.success(`Restored canvas for '${prev}'`);
                })
                .catch((err) => {
                  toast.error('Undo failed', errorMessage(err));
                });
            },
          },
        });
      } else if (seen !== null && profileStore.selectedName !== seen) {
        // Clean canvas, external active swap (schedule fire) — pull the
        // canvas across so it reflects what's now on the desktop.
        const next = profileStore.profiles.find((p) => p.name === seen);
        if (next) {
          profileStore.select(seen);
          applyMonitorStateToCanvas(next);
        }
      }
    }
    preemption.setSentinel(seen);
  });

  // Snapshot the dirty canvas state so the Undo action above has
  // something to restore. Updated on every dirty-canvas frame; cleared
  // when the canvas is clean.
  $effect(() => {
    if (canvasDirty && draft) {
      preemption.setSnapshot($state.snapshot(draft) as Profile);
    } else if (!canvasDirty) {
      preemption.setSnapshot(null);
    }
  });

  seedOverridesFromMonitors(
    () => monitorStore.monitors,
    () => 0,
  );

  useSourceImage(
    () => sourcePath ?? liveSlideshowPath,
    () => previewMonitors,
  );

  const imageUrl = $derived(sourceImageState.value.url);
  const imageNaturalDims = $derived(sourceImageState.value.naturalDims);

  const snapCover = () => runSnapCover(previewMonitors, imageNaturalDims);
  const resetTransform = () => runResetTransform(previewMonitors, imageNaturalDims);
  const resetLayout = () => runResetLayout(snapHmm);

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
      onMonitorsChanged: () => void monitorStore.refresh(),
    });

    // Window-close interception (§4e.11.5). When the canvas is dirty we
    // veto the close, surface the modal, and re-issue the close on
    // confirm. Tauri's `onCloseRequested` handler calls `preventDefault`
    // on the supplied event to abort the close.
    let closeUnlistenFn: (() => void) | null = null;
    const winRef = (() => {
      try {
        return getCurrentWindow();
      } catch {
        return null;
      }
    })();
    if (winRef) {
      void winRef
        .onCloseRequested((event) => {
          if (canvasDirty) {
            event.preventDefault();
            pendingDiscard = {
              label: 'Closing the window',
              perform: () => {
                void winRef.destroy();
              },
            };
          }
        })
        .then((fn) => {
          closeUnlistenFn = fn;
        })
        .catch(() => {
          // ignore — webviews without a window context (tests, etc.)
        });
    }

    window.addEventListener('keydown', onKey);
    void daemonStatus.probe();
    const interval = window.setInterval(() => {
      void profileStore.refresh();
      void slideshowController.refresh();
    }, 5000);
    const daemonInterval = window.setInterval(() => {
      void daemonStatus.probe();
    }, 10_000);

    return () => {
      detachWindow();
      window.clearInterval(interval);
      window.clearInterval(daemonInterval);
      window.removeEventListener('keydown', onKey);
      if (closeUnlistenFn) closeUnlistenFn();
    };
  });

  function setPrimary() {
    if (!canvasView.selectId) return;
    toast.info('Primary change is preview-only', 'will not be pushed to compositor');
  }

  // Source dock metadata — lifted from the active draft for display.
  const sourceName = $derived.by(() => {
    if (sourcePath) return sourcePath.split('/').pop() ?? sourcePath;
    if (slideshowSource) {
      if (liveSlideshowPath) return liveSlideshowPath.split('/').pop() ?? liveSlideshowPath;
      return draft?.name ?? 'slideshow';
    }
    return '— no source';
  });
  const sourceMeta = $derived.by(() => {
    if (slideshowSource) {
      const n = slideshowSource.images.sources.length;
      if (n === 0) return 'slideshow · empty — add images';
      return `slideshow · ${n} source${n === 1 ? '' : 's'}`;
    }
    if (imageNaturalDims) return `${imageNaturalDims.w}×${imageNaturalDims.h}`;
    return sourcePath ? 'loading…' : 'pick from library';
  });
  const backendName = $derived(runtime.last?.backend ?? 'auto-detect');

  const someMissingMm = $derived(
    monitorStore.monitors.length > 0 && monitorStore.monitors.some((m) => !m.physical_size_mm),
  );
</script>

<div class="fixed inset-0 overflow-hidden">
  <PreviewCanvas
    monitors={monitorStore.monitors}
    bezelHmm={snapHmm}
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
    canSaveAsNew={true}
    {canSave}
    {canRevert}
    saveDirty={canvasDirty}
    onApply={() => void applyDraftProfile()}
    onSave={() => void saveActiveProfile()}
    onSaveAsNew={() => (saveDialogOpen = true)}
    onRevert={revertCanvasToActive}
    onSwitchProfile={(p) => guardedDiscard(`Switch to '${p.name}'`, () => pickProfile(p))}
    onOpenLibrary={() => (libraryOpen = true)}
    onOpenSettings={() => (settingsOpen = true)}
    onOpenProfileManager={() => (profileManagerOpen = true)}
    onTrayClick={() => (trayOpen = !trayOpen)}
  />

  {#if saveDialogOpen}
    <SaveProfileDialog
      existingNames={profileStore.profiles.map((p) => p.name)}
      defaultName={profileStore.activeName
        ? `${profileStore.activeName}-copy`
        : `untitled-${profileStore.profiles.length + 1}`}
      hasSource={Boolean(sourcePath)}
      onCancel={() => (saveDialogOpen = false)}
      onConfirm={(n, d, k) => void saveAsNew(n, d, k)}
    />
  {/if}

  <ToolDock onResetTransform={resetTransform} onSnapCover={snapCover} onResetLayout={resetLayout} />

  <MonitorGapDock
    hGapMm={currentHGap}
    vGapMm={currentVGap}
    {hMixed}
    {vMixed}
    fallbackHmm={0}
    fallbackVmm={0}
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
    slideshow={dockSlideshowState}
    slideshowConfig={dockSlideshowProfile?.source.config ?? null}
    onPrev={() => void slideshowController.prev()}
    onNext={() => void slideshowController.next()}
    onTogglePause={() => void slideshowController.togglePause()}
    onUpdateConfig={(c) => void updateSlideshowConfig(c)}
    onOpenLibrary={() => (libraryOpen = true)}
  />

  {#if selectedMonitor}
    <MonitorInspector
      monitor={selectedMonitor}
      {imageUrl}
      imageTransform={imageTransform.value}
      onClose={() => canvasView.setSelectId(null)}
      onSetPrimary={setPrimary}
    />
  {/if}

  {#if !daemonStatus.connected}
    <div class="banner banner-danger">
      <span class="dot danger"></span>
      <span>
        Daemon not running — Apply, Save, and slideshow controls are disabled
        {#if daemonStatus.lastError}
          <span
            class="mono"
            style:font-size="10px"
            style:color="var(--text-3)"
            style:margin-left="6px"
          >
            ({daemonStatus.lastError})
          </span>
        {/if}
      </span>
      <button
        class="btn sm primary"
        disabled={daemonStatus.starting}
        onclick={() => void daemonStatus.start()}
      >
        {daemonStatus.starting ? 'Starting…' : 'Start daemon'}
      </button>
    </div>
  {:else if someMissingMm}
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
      {slideshowTarget}
      onUpdateSlideshow={(images) => void updateSlideshowImages(images)}
    />
  {/if}

  {#if profileManagerOpen}
    <ProfileManagerModal
      onClose={() => (profileManagerOpen = false)}
      onCreateFromCanvas={(n, d, k) => saveAsNew(n, d, k)}
      onEditSlideshow={(p) => {
        profileManagerOpen = false;
        profileStore.select(p.name);
        libraryOpen = true;
      }}
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
        guardedDiscard(`Switch to '${p.name}'`, () => pickProfile(p));
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

  {#if pendingDiscard}
    <ConfirmDiscardModal
      activeName={profileStore.activeName}
      actionLabel={pendingDiscard.label}
      onCancel={() => (pendingDiscard = null)}
      onConfirm={() => {
        const next = pendingDiscard;
        pendingDiscard = null;
        if (next) void next.perform();
      }}
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
  .banner-danger {
    background: color-mix(in oklab, var(--danger) 22%, var(--panel));
    border-color: color-mix(in oklab, var(--danger) 60%, var(--line));
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
