// Out-of-band payload for an internal library→canvas image drag.
//
// WebKitGTK (Tauri's Linux webview) does not preserve the HTML5 `DataTransfer`
// for an element drag whose content includes an `<img>`: it overwrites the data
// with the image's `blob:` object-URL, so a drop reads `blob:.../<uuid>` instead
// of the absolute file path we set via `setData`. Worse, the internal drag is
// delivered through Tauri's *native* drop event too, with that same `blob:` URL
// as the reported "path". We carry the real path here instead — set on
// `dragstart`, consumed by whichever drop handler fires.
let draggedImagePath: string | null = null;
let generation = 0;

export function beginImageDrag(path: string): void {
  generation += 1;
  draggedImagePath = path;
}

/** The dragged image's absolute path, cleared as it is read so the drop is
 *  consumed exactly once. Returns `null` for an OS file-manager drag (no in-app
 *  payload — delivered through Tauri's native file-drop) or once another drop
 *  handler already took it. Both the HTML5 canvas drop and Tauri's native drop
 *  can fire for the same internal drag depending on the WebKitGTK build;
 *  take-once keeps them from adding the image twice. */
export function takeDraggedImagePath(): string | null {
  const path = draggedImagePath;
  draggedImagePath = null;
  return path;
}

/** Safety net for a cancelled drag (no drop consumed the payload). Deferred so a
 *  native Tauri drop — delivered via IPC just after the DOM `dragend` — still
 *  reads the path first, and generation-guarded so a drag that starts during the
 *  delay isn't wiped by the previous one's timer. */
export function endImageDrag(): void {
  const g = generation;
  setTimeout(() => {
    if (g === generation) draggedImagePath = null;
  }, 300);
}
