// Tauri window-event wiring used from `App.svelte` `onMount`. Returns a
// teardown closure that detaches every listener.

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';

export type WindowEventHandlers = {
  onOpenSettings: () => void;
  onDragOver: () => void;
  onDragLeave: () => void;
  onDrop: (path: string) => void;
};

export function attachWindowEvents(handlers: WindowEventHandlers): () => void {
  let unTray: UnlistenFn | undefined;
  let unDrop: UnlistenFn | undefined;

  void listen('tray://open-settings', () => handlers.onOpenSettings()).then((fn) => {
    unTray = fn;
  });

  void getCurrentWebview()
    .onDragDropEvent((ev) => {
      if (ev.payload.type === 'over') handlers.onDragOver();
      else if (ev.payload.type === 'leave') handlers.onDragLeave();
      else if (ev.payload.type === 'drop') {
        handlers.onDragLeave();
        const path = ev.payload.paths[0];
        if (path) handlers.onDrop(path);
      }
    })
    .then((fn) => {
      unDrop = fn;
    });

  return () => {
    unTray?.();
    unDrop?.();
  };
}
