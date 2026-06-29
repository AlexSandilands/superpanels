<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { Profile } from '$lib/api';
  import { canvasView } from '$lib/stores/canvas-view.svelte';
  import { canvasLayers } from '$lib/stores/canvas-layers.svelte';
  import { daemonStatus } from '$lib/stores/daemon-status.svelte';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { preemption } from '$lib/stores/preemption.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { prewarmProfileThumbs } from '$lib/stores/profile-thumbs.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import { applyDocumentTokens } from '$lib/stores/ui.svelte';
  import {
    followSlideshowLayout,
    imageTransform,
    seedOverridesFromMonitors,
    sourceImageState,
    useSourceImage,
  } from '$lib/stores/image-transform.svelte';
  import {
    addImageToCanvas,
    addImageToSlideshowSet,
    applyDraftProfile,
    applyMonitorStateToCanvas,
    dropImageOnMonitor,
    openMainWindow,
    persistSlideshowSource,
    quitApp,
    revertCanvasToActive,
    saveActiveProfile,
    switchAndApply,
  } from '$lib/actions';
  import {
    canvasOverridesDirty,
    coverRectDirty,
    imageTransformDirty,
    liveLayersDirty,
    placementsDirty,
    rectDirty,
  } from '$lib/canvas/dirty';
  import { buildPreviewMonitors, defaultOverrides } from '$lib/canvas/preview-layout';
  import {
    resetLayout as runResetLayout,
    resetSelectedLayer,
    resetTransform as runResetTransform,
    setHGap as runSetHGap,
    setVGap as runSetVGap,
    snapCover as runSnapCover,
    snapLayer,
    snapSelectedLayer,
  } from '$lib/canvas/transform-actions';
  import {
    bbox,
    hNeighbourPairs,
    totalPixels,
    uniformGap,
    vNeighbourPairs,
  } from '$lib/canvas/preview-layout';
  import { removeOverrideForImage, saveOverrideForImage } from '$lib/slideshow-overrides';
  import {
    countAspectMismatches,
    emptyImageSet,
    gcOverrides,
    membershipLookup,
    type AspectMismatch,
  } from '$lib/slideshow-set';
  import { slideshowController } from '$lib/slideshow-controller.svelte';
  import { attachWindowEvents } from '$lib/events/window';
  import { dispatchKey } from '$lib/keymap';
  import {
    defaultSlideshowConfig,
    isSlideshowBody,
    isStandardBody,
    overrideFor,
    type ImageOverride,
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
  import SlideshowDropModal from './components/overlays/SlideshowDropModal.svelte';
  import ConfirmDialog from './components/widgets/ConfirmDialog.svelte';
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

  // Default canvas: detected placements with no gap, image rect over their
  // bounding box. The clean slate a new slideshow starts from.
  function defaultCanvasLayout() {
    const defaults = defaultOverrides(monitorStore.monitors, 0);
    const previews = buildPreviewMonitors(monitorStore.monitors, defaults);
    const bb = bbox(previews);
    return { defaults, previews, bb };
  }

  async function saveAsNew(
    name: string,
    description: string | null,
    kind: ProfileKind = 'standard',
  ) {
    saveDialogOpen = false;
    try {
      let monitor_state: Record<string, { x_mm: number; y_mm: number }> = {};
      let body;
      if (kind === 'slideshow') {
        // A new slideshow starts from a clean slate — empty set, default
        // timer config, default placements. The canvas's image and layout
        // belong to the previous profile and must not bleed in (duplication
        // lives in the profile manager).
        const { previews, bb } = defaultCanvasLayout();
        for (const m of previews) {
          monitor_state[m.id] = { x_mm: m.xMm, y_mm: m.yMm };
        }
        body = {
          type: 'slideshow' as const,
          source: {
            images: emptyImageSet(),
            config: defaultSlideshowConfig(),
          },
          image_rect_mm: { x_mm: bb.x, y_mm: bb.y, w_mm: bb.w, h_mm: bb.h },
        };
      } else {
        // Standard: persist whatever layers are on the canvas (0..N). Switching
        // to it immediately applies them.
        for (const m of previewMonitors) {
          monitor_state[m.id] = { x_mm: m.xMm, y_mm: m.yMm };
        }
        body = { type: 'standard' as const, layers: canvasLayers.toLayers() };
      }
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
        // Reset the live canvas to the clean slate just persisted, then drop
        // straight into the library — the first images added there activate
        // the slideshow (see `updateSlideshowImages`).
        const { defaults, bb } = defaultCanvasLayout();
        canvasView.setOverrides(defaults);
        imageTransform.set({ offsetMmX: bb.x, offsetMmY: bb.y, widthMm: bb.w, heightMm: bb.h });
        profileStore.select(name);
        toast.info('Add images', 'pick images or folders for the slideshow');
        libraryOpen = true;
      } else if (saved) {
        // An empty Standard is valid (it applies as black), but don't black out
        // the desktop just for creating one — select it without applying. With
        // layers present, switch + apply as usual.
        if (isStandardBody(saved.body) && saved.body.layers.length === 0) {
          profileStore.select(name);
          applyMonitorStateToCanvas(saved);
        } else {
          await switchAndApply(saved);
        }
      }
    } catch (err) {
      toast.error('Save as new failed', errorMessage(err));
    }
  }

  applyDocumentTokens();

  const draft = $derived<Profile | null>(profileStore.draft);
  const standard = $derived(draft && isStandardBody(draft.body) ? draft.body : null);
  const slideshow = $derived(draft && isSlideshowBody(draft.body) ? draft.body : null);
  const slideshowSource = $derived(slideshow ? slideshow.source : null);

  // Saved slideshow profile whose image set the library can edit. Unsaved
  // drafts are excluded — `update_profile_source` needs an on-disk profile.
  const slideshowTarget = $derived.by(() => {
    if (!draft || !slideshowSource) return null;
    if (!profileStore.profiles.some((p) => p.name === draft.name)) return null;
    return {
      name: draft.name,
      images: slideshowSource.images,
      overrides: slideshowSource.overrides ?? {},
    };
  });

  // While the active profile is a slideshow, mirror its live image onto the
  // canvas / dock thumb so the preview tracks the desktop.
  const liveSlideshowPath = $derived(
    slideshowSource && draft && draft.name === profileStore.activeName
      ? (slideshowController.state?.currentPath ?? null)
      : null,
  );

  // Per-image canvas override authored for the live slideshow image, if any.
  const liveOverride = $derived<ImageOverride | null>(
    slideshowSource && liveSlideshowPath ? overrideFor(slideshowSource, liveSlideshowPath) : null,
  );

  // The slideshow applies one profile-level layout to every untuned image.
  const uniformLayoutOn = $derived(Boolean(slideshowSource?.uniform_layout));

  // The dock's slideshow controls follow the profile the dock displays (the
  // draft), never whatever happens to be active — editing B while A runs
  // must not silently rewrite A's config.
  const dockSlideshowProfile = $derived(
    slideshowTarget && slideshowSource
      ? { name: slideshowTarget.name, source: slideshowSource }
      : null,
  );

  // Playback state belongs to the active slideshow; hide it while the dock
  // shows a different (merely selected) profile, or once the draft has been
  // converted away from a slideshow (the live runtime keeps rotating until the
  // standard canvas is applied, but its controls shouldn't linger on the dock).
  const dockSlideshowState = $derived(
    draft && isSlideshowBody(draft.body) && draft.name === profileStore.activeName
      ? slideshowController.state
      : null,
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
    const source = $state.snapshot(slideshowSource) as SlideshowSource;
    const hadImages = source.images.sources.length > 0;
    // Editing the set is the natural GC point for overrides orphaned by a
    // removed image or folder source.
    const overrides = gcOverrides(source.overrides, images);
    const ok = await persistSlideshowSource(target.name, { ...source, images, overrides });
    if (!ok) return;
    void slideshowController.refresh();
    // First images for a slideshow that isn't showing anything yet: apply it
    // so the canvas and desktop stop displaying the previous profile's
    // leftovers. Later edits never yank the wallpaper.
    const isActive = profileStore.activeName === target.name;
    if (!isActive && !hadImages && images.sources.length > 0) {
      const saved = profileStore.profiles.find((p) => p.name === target.name);
      if (saved) {
        await switchAndApply(saved);
        void slideshowController.refresh();
      }
    } else if (isActive && !slideshowController.state?.currentPath) {
      await slideshowController.next();
    }
  }

  // Jump the slideshow to a specific image (picked from the library) so the
  // canvas — and the desktop — show it immediately. The daemon's goto only
  // works on the active profile, so a merely-selected slideshow is switched
  // to first.
  async function showImageOnCanvas(path: string) {
    const target = slideshowTarget;
    if (!target) return;
    if (profileStore.activeName !== target.name) {
      const saved = profileStore.profiles.find((p) => p.name === target.name);
      if (!saved) return;
      await switchAndApply(saved);
    }
    await slideshowController.goto(path);
  }

  // Save / reset the canvas as a per-image override for the live slideshow
  // image (logic in `lib/slideshow-overrides.ts`).
  async function saveCanvasForCurrentImage() {
    const target = dockSlideshowProfile;
    const path = liveSlideshowPath;
    if (!target || !path || !draft) return;
    const monitor_state: Record<string, { x_mm: number; y_mm: number }> = {};
    for (const m of previewMonitors) {
      monitor_state[m.id] = { x_mm: m.xMm, y_mm: m.yMm };
    }
    const t = imageTransform.value;
    await saveOverrideForImage(
      target.name,
      $state.snapshot(target.source) as SlideshowSource,
      $state.snapshot(draft) as Profile,
      path,
      {
        monitor_state,
        image_rect_mm: { x_mm: t.offsetMmX, y_mm: t.offsetMmY, w_mm: t.widthMm, h_mm: t.heightMm },
      },
    );
  }

  async function removeImageOverride(path: string) {
    const target = slideshowTarget;
    if (!target || !slideshowSource || !draft) return;
    await removeOverrideForImage(
      target.name,
      $state.snapshot(slideshowSource) as SlideshowSource,
      $state.snapshot(draft) as Profile,
      path,
      liveSlideshowPath,
    );
  }

  // Apply the current canvas to every slideshow image (uniform layout). When
  // the library knows of set images with a different shape, confirm first —
  // those would be stretched into this rect.
  let uniformWarning = $state<AspectMismatch | null>(null);

  function requestApplyToAll(): void {
    const target = dockSlideshowProfile;
    if (!target) return;
    const t = imageTransform.value;
    const counts = countAspectMismatches(
      libraryStore.entries,
      target.source.images,
      t.widthMm / t.heightMm,
    );
    if (counts.mismatched > 0) {
      uniformWarning = counts;
      return;
    }
    void applyCanvasToAllImages();
  }

  async function applyCanvasToAllImages(): Promise<void> {
    uniformWarning = null;
    if (!dockSlideshowProfile) return;
    profileStore.patchDraft((d) => {
      if (isSlideshowBody(d.body)) {
        d.body.source.uniform_layout = true;
      }
    });
    // Save persists the canvas (placements + rect) as the profile layout;
    // the flag rides along in the same write. Then apply so the desktop
    // reflects it without waiting for the next advance.
    const ok = await saveActiveProfile();
    if (!ok) return;
    await applyDraftProfile();
    void slideshowController.refresh();
  }

  async function disableUniformLayout(): Promise<void> {
    const target = dockSlideshowProfile;
    if (!target) return;
    const source = $state.snapshot(target.source) as SlideshowSource;
    const ok = await persistSlideshowSource(target.name, { ...source, uniform_layout: false });
    if (!ok) return;
    toast.success('Uniform layout off', 'untuned images auto-fit at their own aspect');
    if (liveSlideshowPath && !liveOverride) {
      snapCover();
      await applyDraftProfile();
    }
  }

  // An empty slideshow can't be applied (its pool is empty and validity
  // disables it) — switching to one selects it and opens the library to
  // populate the set instead of surfacing an apply failure.
  function pickProfile(p: Profile) {
    const emptySlideshow = isSlideshowBody(p.body) && p.body.source.images.sources.length === 0;
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

  // An empty Standard canvas is a valid state — applying it just paints every
  // monitor black — so Apply stays enabled (unlike the empty-slideshow gate).
  const canApply = $derived(Boolean(draft && draft.name.trim() && !profileStore.saving));

  // Dirty detection (§4e.11.3). The store's own `dirty` flag covers
  // explicit `patchDraft` calls (image source, body shape); on top of that we
  // diff the live canvas state — monitor overrides + image transform — against
  // the active profile so drag / rotate / image-resize edits show up here too.
  // When the live slideshow image carries a per-image override, that override
  // is the baseline instead: matching its saved layout is clean.
  const canvasDirty = $derived.by(() => {
    if (profileStore.dirty) return true;
    const active = profileStore.activeProfile;
    if (!active) return false;
    if (standard) {
      if (!isStandardBody(active.body)) return true;
      if (canvasOverridesDirty(canvasView.overrides, active)) return true;
      return liveLayersDirty(canvasLayers.list, active.body.layers);
    }
    // While a slideshow image is still resolving, the transform (and loaded
    // dims) belong to the previous image — rect comparisons against the
    // incoming image's baseline would flag phantom edits.
    const imageSettled = !liveSlideshowPath || sourceImageState.value.path === liveSlideshowPath;
    if (liveOverride) {
      if (placementsDirty(canvasView.overrides, liveOverride.monitor_state)) return true;
      return imageSettled && rectDirty(imageTransform.value, liveOverride.image_rect_mm);
    }
    if (canvasOverridesDirty(canvasView.overrides, active)) return true;
    if (!imageSettled) return false;
    if (liveSlideshowPath && !uniformLayoutOn) {
      // Untuned slideshow image: clean means the cover-fit seed, not the
      // profile rect (which belongs to a different image's aspect). Under a
      // uniform layout the profile rect IS the baseline — fall through.
      return coverRectDirty(
        imageTransform.value,
        previewMonitors,
        sourceImageState.value.naturalDims,
      );
    }
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

  // Adding an image while editing a slideshow would silently discard its
  // source/timer/overrides; offer Add-to-set vs Convert-to-standard instead.
  // Bound to PreviewCanvas so the OS file-drop path can hit-test the drop point
  // against the monitors — Tauri's native drop bypasses the canvas's own
  // `ondrop` handler, so it can't reuse that path's monitor targeting.
  let canvasRef = $state<{ monitorIdAtClient: (x: number, y: number) => string | null }>();

  let pendingCanvasDrop = $state<{ path: string; monitorId?: string } | null>(null);

  function requestAddImageToCanvas(path: string, monitorId?: string): void {
    if (draft && isSlideshowBody(draft.body)) {
      pendingCanvasDrop = monitorId ? { path, monitorId } : { path };
      return;
    }
    if (monitorId) dropImageOnMonitor(monitorId, path);
    else addImageToCanvas(path);
  }

  // Canvas / library drops that target a specific monitor add a layer pre-snapped
  // to fill it; they still route through the slideshow-discard guard above.
  function requestDropOnMonitor(monitorId: string, path: string): void {
    requestAddImageToCanvas(path, monitorId);
  }

  // Schedule-preemption tracking (§4e.11.6). The sentinel + dirty-canvas
  // snapshot live in `lib/stores/preemption.svelte` so user-driven actions
  // (`applyDraftProfile`, `saveActiveProfile`, `switchAndApply`) can claim
  // an upcoming switch and avoid being misread as a schedule fire.
  $effect(() => {
    const seen = profileStore.activeName;
    const sentinel = preemption.sentinel;
    const pending = preemption.pendingClaim;
    if (pending) {
      if (seen === pending.to) {
        // The user's own switch landed in the runtime view.
        preemption.settleClaim();
        return;
      }
      if (seen === pending.from) {
        // A poll that raced the switch still reports the old name — a stale
        // echo, not a schedule fire. Keep waiting for the claim to land.
        return;
      }
      // A third profile became active mid-switch: a genuine schedule fire
      // overtook the user's switch — handle it as an external change.
      preemption.settleClaim();
    }
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

  // Background-fetch switcher/tray thumbnails so the menus open warm.
  $effect(() => {
    prewarmProfileThumbs(
      profileStore.profiles,
      libraryStore.entries.map((e) => e.path),
    );
  });

  const rectToTransform = (r: { x_mm: number; y_mm: number; w_mm: number; h_mm: number }) => ({
    offsetMmX: r.x_mm,
    offsetMmY: r.y_mm,
    widthMm: r.w_mm,
    heightMm: r.h_mm,
  });

  // The span-image machinery now drives only the slideshow's live image.
  useSourceImage(
    () => liveSlideshowPath,
    () => previewMonitors,
    // The authored rect for a slideshow image is its per-image override, or the
    // profile-level rect when the slideshow runs one uniform layout. Everything
    // else — untuned slideshow images — falls through to the cover-fit seed.
    (path) => {
      if (path !== liveSlideshowPath) return null;
      const active = profileStore.activeProfile;
      const r =
        liveOverride?.image_rect_mm ??
        (uniformLayoutOn && active && isSlideshowBody(active.body)
          ? active.body.image_rect_mm
          : null);
      return r ? rectToTransform(r) : null;
    },
  );

  // The canvas follows per-image overrides live: on slideshow advance,
  // re-gap and re-place to the new image's effective layout.
  followSlideshowLayout(
    () => liveSlideshowPath,
    () => liveOverride?.monitor_state ?? profileStore.activeProfile?.monitor_state ?? null,
  );

  const imageUrl = $derived(sourceImageState.value.url);
  const imageNaturalDims = $derived(sourceImageState.value.naturalDims);

  // Snap / reset target the selected layer in a standard profile; for a
  // slideshow they fall back to the single span image (cover / reset only —
  // single-axis snaps are layer-only).
  const snapWidth = () => snapSelectedLayer(previewMonitors, 'width');
  const snapHeight = () => snapSelectedLayer(previewMonitors, 'height');
  const snapCover = () =>
    standard
      ? snapSelectedLayer(previewMonitors, 'cover')
      : runSnapCover(previewMonitors, imageNaturalDims);
  const resetTransform = () =>
    standard
      ? resetSelectedLayer(previewMonitors)
      : runResetTransform(previewMonitors, imageNaturalDims);
  const resetLayout = () => runResetLayout(snapHmm);

  const snapEnabled = $derived(
    standard ? Boolean(canvasView.selectedLayerId) : Boolean(imageNaturalDims),
  );
  const axisSnapEnabled = $derived(Boolean(standard && canvasView.selectedLayerId));

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

  // Cold-start canvas seeding: pull the active profile's placements (monitor
  // gaps) and single-source rect into the canvas. An effect rather than a
  // one-shot in onMount because the first `current_state` polls can race the
  // daemon's own startup restore (it re-applies its resume/default profile
  // shortly after boot) — the active profile may only arrive a poll later.
  // Unconditional on the first active: `seedOverridesFromMonitors` fills
  // overrides with zero-gap defaults as soon as detection lands, so the
  // canvas is never "untouched" to test against — and the seed merges over
  // those defaults safely in either arrival order. Guarding on `canvasDirty`
  // is circular: an unseeded canvas always diffs dirty against its profile.
  let canvasSeeded = false;
  $effect(() => {
    const active = profileStore.activeProfile;
    if (canvasSeeded || !active) return;
    canvasSeeded = true;
    applyMonitorStateToCanvas(active);
  });

  onMount(() => {
    void monitorStore.refresh();
    void profileStore.refresh().then(() => {
      void slideshowController.refresh();
    });
    void libraryStore.refresh();

    const detachWindow = attachWindowEvents({
      onOpenSettings: () => (settingsOpen = true),
      onDragOver: () => (dragOverlay = true),
      onDragLeave: () => (dragOverlay = false),
      onDrop: (path, position) => {
        // Tauri reports the drop in physical pixels; the canvas hit-test works
        // in CSS pixels relative to the (viewport-filling) stage.
        const dpr = window.devicePixelRatio || 1;
        const monitorId =
          canvasRef?.monitorIdAtClient(position.x / dpr, position.y / dpr) ?? undefined;
        requestAddImageToCanvas(path, monitorId);
      },
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
    void daemonStatus.probe().then(() => {
      // Bring up the daemon when it isn't running — Apply and slideshow
      // control have no in-process fallback, and a daemon restored this way
      // resumes the last-active profile by itself.
      if (!daemonStatus.connected) void daemonStatus.start();
    });
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

  // Source dock metadata — lifted from the active draft for display.
  const sourceName = $derived.by(() => {
    if (standard) {
      const n = canvasLayers.list.length;
      if (n === 1) {
        const p = canvasLayers.list[0]?.path;
        if (p) return p.split('/').pop() ?? p;
      }
      return draft?.name ?? 'standard';
    }
    if (slideshowSource) {
      if (liveSlideshowPath) return liveSlideshowPath.split('/').pop() ?? liveSlideshowPath;
      return draft?.name ?? 'slideshow';
    }
    return '— no source';
  });
  const sourceMeta = $derived.by(() => {
    if (standard) {
      const n = canvasLayers.list.length;
      if (n === 0) return 'standard · empty — add images';
      return `standard · ${n} image${n === 1 ? '' : 's'}`;
    }
    if (slideshowSource) {
      const n = slideshowSource.images.sources.length;
      if (n === 0) return 'slideshow · empty — add images';
      return `slideshow · ${n} source${n === 1 ? '' : 's'}`;
    }
    if (imageNaturalDims) return `${imageNaturalDims.w}×${imageNaturalDims.h}`;
    return 'pick from library';
  });
  // The dock swatch tracks the live slideshow image, or — on a Standard canvas,
  // which has no single `imageUrl` — the top (last) layer's resolved image.
  const dockThumbUrl = $derived(standard ? (canvasLayers.list.at(-1)?.url ?? null) : imageUrl);
  // Layers feeding the monitor inspector's crop preview, bottom-to-top: every
  // Standard layer (so each monitor shows whichever images actually cover it),
  // or the single live slideshow image.
  const inspectorLayers = $derived.by(() => {
    if (standard) return canvasLayers.list.map((l) => ({ url: l.url, transform: l.transform }));
    if (imageUrl) return [{ url: imageUrl, transform: imageTransform.value }];
    return [];
  });
  // Library images belonging to the active slideshow set — the quick-jump grid.
  // (Pool images not indexed in the library aren't shown; slideshow sources are
  // library folders, so coverage is near-complete in practice.)
  const slideshowJumpImages = $derived.by(() => {
    if (!slideshowSource) return [];
    const member = membershipLookup(slideshowSource.images);
    return libraryStore.entries
      .filter((e) => member(e.path) !== null)
      .map((e) => e.path)
      .sort((a, b) => a.localeCompare(b));
  });
  const backendName = $derived(runtime.last?.backend ?? 'auto-detect');

  const someMissingMm = $derived(
    monitorStore.monitors.length > 0 && monitorStore.monitors.some((m) => !m.physical_size_mm),
  );
</script>

<div class="fixed inset-0 overflow-hidden">
  <PreviewCanvas
    bind:this={canvasRef}
    monitors={monitorStore.monitors}
    bezelHmm={snapHmm}
    imageUrl={standard ? null : imageUrl}
    imageTransform={imageTransform.value}
    onImageTransformChange={(t) => imageTransform.set(t)}
    onMonitorDrop={requestDropOnMonitor}
    onCanvasDrop={(path) => requestAddImageToCanvas(path)}
    layers={standard ? canvasLayers.list : []}
    onLayerTransformChange={(id, t) => canvasLayers.patch(id, t)}
    onLayerRemove={(id) => canvasLayers.remove(id)}
    onLayerSelect={(id) => canvasLayers.bringToFront(id)}
    onLayerSnap={(id, axis) => snapLayer(id, previewMonitors, axis)}
  />

  <ModeHint slideshowActive={Boolean(dockSlideshowState || dockSlideshowProfile)} />

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
      onCancel={() => (saveDialogOpen = false)}
      onConfirm={(n, d, k) => void saveAsNew(n, d, k)}
    />
  {/if}

  <ToolDock
    mode={canvasView.mode}
    onSetMode={(m) => canvasView.setMode(m)}
    onSnapWidth={snapWidth}
    onSnapHeight={snapHeight}
    onSnapCover={snapCover}
    onResetTransform={resetTransform}
    onResetLayout={resetLayout}
    {snapEnabled}
    {axisSnapEnabled}
  />

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
    sourceThumbUrl={dockThumbUrl}
    jumpImages={slideshowJumpImages}
    currentImagePath={liveSlideshowPath}
    onJump={(p) => void slideshowController.goto(p)}
    slideshow={dockSlideshowState}
    slideshowConfig={dockSlideshowProfile?.source.config ?? null}
    canSaveForImage={Boolean(dockSlideshowProfile && liveSlideshowPath)}
    hasImageOverride={Boolean(liveOverride)}
    {uniformLayoutOn}
    onPrev={() => void slideshowController.prev()}
    onNext={() => void slideshowController.next()}
    onTogglePause={() => void slideshowController.togglePause()}
    onUpdateConfig={(c) => void updateSlideshowConfig(c)}
    onSaveForImage={() => void saveCanvasForCurrentImage()}
    onResetForImage={() => {
      if (liveSlideshowPath) void removeImageOverride(liveSlideshowPath);
    }}
    onApplyToAll={requestApplyToAll}
    onResetUniform={() => void disableUniformLayout()}
    onOpenLibrary={() => (libraryOpen = true)}
  />

  {#if uniformWarning}
    <ConfirmDialog
      title="Apply this layout to all images?"
      body={`${uniformWarning.mismatched} of ${uniformWarning.known} images in this slideshow have a different aspect ratio and may look stretched under one shared layout. Images with their own saved layout are not affected.`}
      confirmLabel="Apply to all"
      onCancel={() => (uniformWarning = null)}
      onConfirm={() => void applyCanvasToAllImages()}
    />
  {/if}

  {#if selectedMonitor}
    <!-- Monitor details (resolution, physical size, position). The crop preview
         shows the live slideshow image, or the top Standard layer; it renders
         image-less when neither exists, which is fine. -->
    <MonitorInspector
      monitor={selectedMonitor}
      layers={inspectorLayers}
      onClose={() => canvasView.setSelectId(null)}
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
      onPinToMonitor={requestDropOnMonitor}
      onAddToCanvas={requestAddImageToCanvas}
      onImageDragStart={() => (libraryOpen = false)}
      {slideshowTarget}
      onUpdateSlideshow={(images) => void updateSlideshowImages(images)}
      onResetOverride={(path) => void removeImageOverride(path)}
      onShowImage={(path) => void showImageOnCanvas(path)}
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

  {#if pendingCanvasDrop}
    <SlideshowDropModal
      fileName={pendingCanvasDrop.path.split('/').pop() ?? pendingCanvasDrop.path}
      onCancel={() => (pendingCanvasDrop = null)}
      onConvert={() => {
        const next = pendingCanvasDrop;
        pendingCanvasDrop = null;
        if (!next) return;
        if (next.monitorId) dropImageOnMonitor(next.monitorId, next.path);
        else addImageToCanvas(next.path);
      }}
      onAddToSet={() => {
        const next = pendingCanvasDrop;
        pendingCanvasDrop = null;
        if (next) void addImageToSlideshowSet(next.path);
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
