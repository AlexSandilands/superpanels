// Svelte action that reparents an element to <body>. Overlays rendered
// inside a `.panel` need this: the panel's backdrop-filter makes it a
// containing block for fixed descendants, so backdrops/menus would clip to
// the panel instead of covering the viewport.
export function portal(node: HTMLElement): { destroy(): void } {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}
