// Keyboard shortcut dispatch. Calls into module-level action helpers for
// store-level mutations and bounces UI-state mutations back to the caller via
// the small `Overlays` object so the component remains the source of truth
// for its modal flags.

import { canvasView } from '$lib/stores/canvas-view.svelte';
import { canvasLayers } from '$lib/stores/canvas-layers.svelte';
import { profileStore } from '$lib/stores/profile.svelte';
import { nudgeSelected } from '$lib/canvas/select';
import { applyDraftProfile, redetectMonitorsWithToast, switchAndApply } from '$lib/actions';
import { slideshowController } from '$lib/slideshow-controller.svelte';

export type Overlays = {
  libraryOpen: boolean;
  settingsOpen: boolean;
  trayOpen: boolean;
  setLibraryOpen: (v: boolean) => void;
  setSettingsOpen: (v: boolean) => void;
  setTrayOpen: (v: boolean) => void;
};

export type KeymapDeps = {
  overlays: Overlays;
  canApply: boolean;
  resetTransform: () => void;
};

function isInputTarget(e: KeyboardEvent): boolean {
  return ['INPUT', 'TEXTAREA', 'SELECT'].includes((e.target as HTMLElement)?.tagName ?? '');
}

export function dispatchKey(e: KeyboardEvent, deps: KeymapDeps): void {
  const { overlays, canApply, resetTransform } = deps;
  const modal = overlays.libraryOpen || overlays.settingsOpen;

  if (e.key === 'Escape') {
    if (overlays.settingsOpen) overlays.setSettingsOpen(false);
    else if (overlays.libraryOpen) overlays.setLibraryOpen(false);
    else if (overlays.trayOpen) overlays.setTrayOpen(false);
    else if (canvasView.selectId) canvasView.setSelectId(null);
    return;
  }
  if (isInputTarget(e)) return;
  if ((e.key === 'Delete' || e.key === 'Backspace') && !modal && canvasView.selectedLayerId) {
    e.preventDefault();
    canvasLayers.remove(canvasView.selectedLayerId);
    canvasView.setSelectedLayerId(null);
    return;
  }
  if (e.key === 'Enter' && !modal) {
    if (canApply) void applyDraftProfile();
    return;
  }
  if ((e.metaKey || e.ctrlKey) && e.key === ',') {
    e.preventDefault();
    overlays.setSettingsOpen(true);
    return;
  }
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'l') {
    e.preventDefault();
    overlays.setLibraryOpen(true);
    return;
  }
  if ((e.metaKey || e.ctrlKey) && /^[123]$/.test(e.key)) {
    e.preventDefault();
    const p = profileStore.profiles[Number.parseInt(e.key, 10) - 1];
    if (p) void switchAndApply(p);
    return;
  }
  if (e.key === ' ') {
    e.preventDefault();
    void slideshowController.togglePause();
    return;
  }
  if (e.key === 'ArrowRight' && !modal) void slideshowController.next();
  if (e.key === 'ArrowLeft' && !modal) void slideshowController.prev();
  if (e.key.toLowerCase() === 'r' && !modal) resetTransform();
  if (e.key.toLowerCase() === 'd' && !modal) canvasView.toggleDim();
  if (e.key.toLowerCase() === 'm' && !modal) canvasView.toggleMode();
  if (e.key === 'F5') {
    e.preventDefault();
    void redetectMonitorsWithToast();
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
      nudgeSelected(d[0], d[1]);
    }
  }
}
