// Tauri window-event wiring used from `App.svelte` `onMount`. Returns a
// teardown closure that detaches every listener.

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';

export type WindowEventHandlers = {
  onOpenSettings: () => void;
  onDragOver: () => void;
  onDragLeave: () => void;
  /** `position` is the drop point in physical pixels relative to the webview. */
  onDrop: (path: string, position: { x: number; y: number }) => void;
  /** Daemon-driven OS-rotation push (KDE kscreen `configChanged`). */
  onMonitorsChanged: () => void;
};

export function attachWindowEvents(handlers: WindowEventHandlers): () => void {
  let unTray: UnlistenFn | undefined;
  let unDrop: UnlistenFn | undefined;
  let unMonitors: UnlistenFn | undefined;

  void listen('tray://open-settings', () => handlers.onOpenSettings()).then((fn) => {
    unTray = fn;
  });

  void listen('monitors://changed', () => handlers.onMonitorsChanged()).then((fn) => {
    unMonitors = fn;
  });

  void getCurrentWebview()
    .onDragDropEvent((ev) => {
      if (ev.payload.type === 'over') handlers.onDragOver();
      else if (ev.payload.type === 'leave') handlers.onDragLeave();
      else if (ev.payload.type === 'drop') {
        handlers.onDragLeave();
        const path = ev.payload.paths[0];
        if (path) handlers.onDrop(path, ev.payload.position);
      }
    })
    .then((fn) => {
      unDrop = fn;
    });

  return () => {
    unTray?.();
    unDrop?.();
    unMonitors?.();
  };
}
