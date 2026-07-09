<script lang="ts">
  // Resize cursors for the undecorated window. The resize itself is started in
  // Rust (`crates/superpanels-gui/src/resize_borders.rs`), which intercepts the
  // GTK button press before the webview ever sees it — these elements only make
  // the grab regions discoverable, and shadow the titlebar so its top edge shows
  // a resize cursor rather than the move one.
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const win = (() => {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  })();

  let maximized = $state(false);

  $effect(() => {
    if (!win) return;
    void win.isMaximized().then((v) => (maximized = v));
    const unlisten = win.onResized(() => {
      void win.isMaximized().then((v) => (maximized = v));
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  // Mirrors `edge_band` / `corner_band` in `resize_borders.rs`, including its
  // widening on integer-scaled displays — a cursor that doesn't match where the
  // grab actually starts is worse than no cursor.
  const scale = $derived(Math.max(1, Math.floor(window.devicePixelRatio)));
  const edgePx = $derived(Math.max(6, scale * 5 + 1));
  const cornerPx = $derived(Math.max(18, edgePx + 12));
</script>

{#if !maximized}
  <div class="grips" style:--edge="{edgePx}px" style:--corner="{cornerPx}px">
    <div class="grip n"></div>
    <div class="grip s"></div>
    <div class="grip w"></div>
    <div class="grip e"></div>
    <div class="grip nw"></div>
    <div class="grip ne"></div>
    <div class="grip sw"></div>
    <div class="grip se"></div>
  </div>
{/if}

<style>
  /* Above the titlebar (z-10) so the top edge and corners read as resize
     handles; below popovers (41), toasts (50) and modals. */
  .grip {
    position: fixed;
    z-index: 30;
  }
  .n,
  .s {
    left: var(--corner);
    right: var(--corner);
    height: var(--edge);
    cursor: ns-resize;
  }
  .n {
    top: 0;
  }
  .s {
    bottom: 0;
  }
  .w,
  .e {
    top: var(--corner);
    bottom: var(--corner);
    width: var(--edge);
    cursor: ew-resize;
  }
  .w {
    left: 0;
  }
  .e {
    right: 0;
  }
  .nw,
  .ne,
  .sw,
  .se {
    width: var(--corner);
    height: var(--corner);
  }
  .nw {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }
  .ne {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }
  .sw {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
  }
  .se {
    bottom: 0;
    right: 0;
    cursor: nwse-resize;
  }
</style>
