<script lang="ts">
  // SPEC §12.3 monitor preview canvas. Five-layer compositing handled by the
  // pure draw module; pointer/wheel/keyboard wiring + drag-state machine live
  // in `lib/canvas/interaction.svelte.ts`. This component owns the canvas
  // element, layout derivation, and the requestAnimationFrame loop.

  import { onMount } from 'svelte';
  import type { Monitor } from '$lib/api';
  import { computeLayout } from '$lib/canvas/layout';
  import { drawCanvasLayers } from '$lib/canvas/draw';
  import type { CanvasLayout, MonitorRect } from '$lib/canvas/types';
  import { loadThumbnail, peekThumbnail } from '$lib/canvas/image_cache';
  import { CanvasInteraction } from '$lib/canvas/interaction.svelte';
  import MonitorPopout from './MonitorPopout.svelte';

  type Props = {
    monitors: Monitor[];
    bezelHmm: number;
    bezelVmm: number;
    fit: 'fill' | 'fit' | 'stretch' | 'center';
    imagePath: string | null;
    offset: [number, number];
    onOffsetCommit?: (offset: [number, number]) => void;
    onResetOffset?: () => void;
    onMonitorClick?: (rect: MonitorRect) => void;
    onImageLoadError?: (path: string, message: string) => void;
    flashIndices?: number[];
  };

  let {
    monitors,
    bezelHmm,
    bezelVmm,
    fit,
    imagePath,
    offset,
    onOffsetCommit,
    onResetOffset,
    onMonitorClick,
    onImageLoadError,
    flashIndices = [],
  }: Props = $props();

  let wrapperEl: HTMLDivElement | undefined = $state();
  let canvasEl: HTMLCanvasElement | undefined = $state();

  let viewportW = $state(800);
  let viewportH = $state(450);
  let dpr = $state(1);

  let image = $state<HTMLImageElement | null>(null);
  let imageLoadingPath: string | null = null;
  let imageLoading = $state(false);

  const padding = 28;

  // The interaction instance owns the runes for zoom, hover, drag, and
  // popout. It needs read access to the *current* layout, offset, and
  // offset-scale; we hand it lazy getters so it always sees the latest
  // derived values. Declared first so the layout can read `interaction.zoom`;
  // the getter return-type annotations break the type-inference cycle TS
  // would otherwise flag in strict mode.
  const interaction = new CanvasInteraction({
    getLayout: (): CanvasLayout => layout,
    getOffset: (): [number, number] => offset,
    getOffsetScale: (): number => offsetScale,
    onOffsetCommit: (o) => onOffsetCommit?.(o),
    onResetOffset: () => onResetOffset?.(),
    onMonitorClick: (r) => onMonitorClick?.(r),
  });

  const layout: CanvasLayout = $derived(
    computeLayout({
      monitors,
      bezelHmm,
      bezelVmm,
      viewportW,
      viewportH,
      padding,
      zoom: interaction.zoom,
    }),
  );

  const offsetScale: number = $derived(layout.mmToPx / layout.coreMmToPx);
  const displayOffset = $derived<[number, number]>([
    offset[0] * offsetScale,
    offset[1] * offsetScale,
  ]);
  const liveDisplayOffset = $derived<[number, number]>([
    displayOffset[0] + interaction.dragLiveDelta[0],
    displayOffset[1] + interaction.dragLiveDelta[1],
  ]);

  $effect(() => {
    const path = imagePath;
    if (!path) {
      image = null;
      imageLoadingPath = null;
      imageLoading = false;
      return;
    }
    const cached = peekThumbnail(path);
    if (cached) {
      image = cached;
      imageLoadingPath = null;
      imageLoading = false;
      return;
    }
    imageLoadingPath = path;
    imageLoading = true;
    void loadThumbnail(path)
      .then((img) => {
        if (imageLoadingPath === path) {
          image = img;
          imageLoading = false;
        }
      })
      .catch((err: unknown) => {
        if (imageLoadingPath === path) {
          image = null;
          imageLoading = false;
          const message = err instanceof Error ? err.message : String(err);
          onImageLoadError?.(path, message);
        }
      });
  });

  let pendingFrame = false;

  $effect(() => {
    // Touch reactive deps so the effect re-runs on changes. The reads must be
    // synchronous — the rAF callback below runs after Svelte's tracking window
    // closes, so reading inside it would not re-subscribe.
    void layout;
    void liveDisplayOffset;
    void interaction.hoverIndex;
    void image;
    void flashIndices;
    void fit;
    void dpr;
    if (pendingFrame) return;
    pendingFrame = true;
    requestAnimationFrame(() => {
      pendingFrame = false;
      paint();
    });
  });

  function syncCanvasSize() {
    if (!wrapperEl || !canvasEl) return;
    const rect = wrapperEl.getBoundingClientRect();
    viewportW = Math.max(1, Math.floor(rect.width));
    viewportH = Math.max(1, Math.floor(rect.height));
    dpr = window.devicePixelRatio || 1;
    canvasEl.width = Math.round(viewportW * dpr);
    canvasEl.height = Math.round(viewportH * dpr);
  }

  onMount(() => {
    if (!wrapperEl || !canvasEl) return;
    syncCanvasSize();
    const observer = new ResizeObserver(syncCanvasSize);
    observer.observe(wrapperEl);

    // DPR can change without firing a resize — moving the window between
    // monitors of different scale, or the user zooming the webview. The
    // standard hook is a `(resolution: ${dpr}dppx)` MQ that re-fires on every
    // change; re-arm it after each fire because the MQ string is dpr-bound.
    let dprMql: MediaQueryList | null = null;
    function watchDpr() {
      dprMql?.removeEventListener('change', onDprChange);
      dprMql = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      dprMql.addEventListener('change', onDprChange);
    }
    function onDprChange() {
      syncCanvasSize();
      watchDpr();
    }
    watchDpr();

    const onKeyDown = (ev: KeyboardEvent) => interaction.onKey(ev);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      observer.disconnect();
      dprMql?.removeEventListener('change', onDprChange);
      window.removeEventListener('keydown', onKeyDown);
    };
  });

  function paint() {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext('2d');
    if (!ctx) return;
    const t0 = paintInstrumentation ? performance.now() : 0;
    drawCanvasLayers(ctx, layout, {
      dpr,
      viewportW,
      viewportH,
      image,
      imageW: image?.naturalWidth ?? 0,
      imageH: image?.naturalHeight ?? 0,
      offsetX: liveDisplayOffset[0],
      offsetY: liveDisplayOffset[1],
      fit,
      hoverIndex: interaction.hoverIndex,
      showLabels: interaction.zoom >= 0.7,
    });
    drawFlash(ctx);
    if (paintInstrumentation) recordPaint(performance.now() - t0);
  }

  // Opt-in baseline-capture hook for SPEC §19's "Canvas drag → redraw frame"
  // budget (< 8 ms). Toggle via `localStorage.setItem('superpanels.bench', '1')`
  // in the webview console; results land on `window.__superpanelsPaint`.
  const paintInstrumentation =
    typeof window !== 'undefined' && window.localStorage?.getItem('superpanels.bench') === '1';

  function recordPaint(ms: number) {
    const w = window as Window & { __superpanelsPaint?: number[] };
    if (!w.__superpanelsPaint) w.__superpanelsPaint = [];
    w.__superpanelsPaint.push(ms);
    if (w.__superpanelsPaint.length > 240) w.__superpanelsPaint.shift();
  }

  function drawFlash(ctx: CanvasRenderingContext2D) {
    if (flashIndices.length === 0) return;
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.globalAlpha = 0.55;
    ctx.fillStyle = '#bae6fd';
    for (const idx of flashIndices) {
      const m = layout.monitors.find((r) => r.monitorIndex === idx);
      if (m) ctx.fillRect(m.x, m.y, m.w, m.h);
    }
    ctx.restore();
  }

  const popoutRect = $derived<MonitorRect | null>(interaction.popoutRect());
</script>

<div
  bind:this={wrapperEl}
  class="relative h-full w-full overflow-hidden rounded border border-slate-800 bg-bezel"
>
  <canvas
    bind:this={canvasEl}
    class="absolute inset-0 h-full w-full select-none"
    style="touch-action: none;"
    style:cursor={interaction.dragging
      ? 'grabbing'
      : interaction.hoverIndex !== null
        ? 'pointer'
        : 'grab'}
    onpointermove={(ev) => canvasEl && interaction.onPointerMove(ev, canvasEl)}
    onpointerdown={(ev) => canvasEl && interaction.onPointerDown(ev, canvasEl)}
    onpointerup={(ev) => canvasEl && interaction.onPointerUp(ev, canvasEl)}
    onpointercancel={(ev) => canvasEl && interaction.onPointerUp(ev, canvasEl)}
    onpointerleave={() => interaction.onPointerLeave()}
    onwheel={(ev) => interaction.onWheel(ev)}
    aria-label="Monitor preview canvas"
  ></canvas>

  {#if monitors.length === 0}
    <div
      class="pointer-events-none absolute inset-0 flex items-center justify-center text-sm text-slate-500"
    >
      No monitors detected — try
      <code class="ml-1 rounded bg-slate-800 px-1">superpanels detect --debug</code>
    </div>
  {:else if !imagePath}
    <div
      class="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center text-xs text-slate-500"
    >
      Drop an image here or pick one from the library to preview the layout.
    </div>
  {:else if imageLoading}
    <div
      class="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center text-xs text-slate-400"
    >
      Loading thumbnail…
    </div>
  {/if}

  <div
    class="pointer-events-none absolute right-2 top-2 rounded bg-slate-900/70 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-slate-400"
  >
    zoom {(interaction.zoom * 100).toFixed(0)}%
  </div>

  {#if popoutRect}
    <MonitorPopout
      {layout}
      rect={popoutRect}
      {image}
      {fit}
      offset={liveDisplayOffset}
      onClose={() => interaction.closePopout()}
    />
  {/if}
</div>
