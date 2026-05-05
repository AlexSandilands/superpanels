<script lang="ts">
  // SPEC §12.3 monitor preview canvas. Phase 4c: free-positioning model with
  // pan + corner resize + off-monitor dim. The pure draw module owns the
  // 7-layer compositing; pointer/wheel/keyboard wiring + drag-state machine
  // live in `lib/canvas/interaction.svelte.ts`.

  import { onMount } from 'svelte';
  import type { Monitor } from '$lib/api';
  import { computeLayout } from '$lib/canvas/layout';
  import { drawCanvasLayers, computeImageRect, hitTest } from '$lib/canvas/draw';
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
    imageSizePx: [number, number] | null;
    onTransformCommit?: (offset: [number, number], imageSizePx: [number, number] | null) => void;
    onResetTransform?: () => void;
    onMonitorClick?: (rect: MonitorRect) => void;
    onMonitorDrop?: (monitorIndex: number, path: string) => void;
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
    imageSizePx,
    onTransformCommit,
    onResetTransform,
    onMonitorClick,
    onMonitorDrop,
    onImageLoadError,
    flashIndices = [],
  }: Props = $props();

  let dropHoverIdx = $state<number | null>(null);
  // Local-only UI state. Aspect-lock defaults to on (the common case for
  // photographs); the user can release it to stretch.
  let aspectLock = $state(true);

  let wrapperEl: HTMLDivElement | undefined = $state();
  let canvasEl: HTMLCanvasElement | undefined = $state();

  let viewportW = $state(800);
  let viewportH = $state(450);
  let dpr = $state(1);

  let image = $state<HTMLImageElement | null>(null);
  let imageLoadingPath: string | null = null;
  let imageLoading = $state(false);
  let hoverCorner = $state<'tl' | 'tr' | 'bl' | 'br' | null>(null);

  const padding = 28;

  type ImgRect = { x: number; y: number; w: number; h: number } | null;

  const interaction = new CanvasInteraction({
    getLayout: (): CanvasLayout => layout,
    getOffset: (): [number, number] => offset,
    getOffsetScale: (): number => offsetScale,
    getImageRect: (): ImgRect => imageRectDisplay,
    getImageSizePx: (): [number, number] | null => imageSizePx,
    getAspectLock: (): boolean => aspectLock,
    onTransformCommit: (o, s) => onTransformCommit?.(o, s),
    onResetTransform: () => onResetTransform?.(),
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
  const imageSizeDisplayPx = $derived<[number, number] | null>(
    imageSizePx ? [imageSizePx[0] * offsetScale, imageSizePx[1] * offsetScale] : null,
  );
  const liveDisplayOffset = $derived<[number, number]>([
    displayOffset[0] + (interaction.dragMode?.kind === 'pan' ? interaction.dragLiveDelta[0] : 0),
    displayOffset[1] + (interaction.dragMode?.kind === 'pan' ? interaction.dragLiveDelta[1] : 0),
  ]);

  const imageRectDisplay: ImgRect = $derived(
    image
      ? computeImageRect(layout, {
          dpr,
          viewportW,
          viewportH,
          image,
          imageW: image.naturalWidth,
          imageH: image.naturalHeight,
          offsetX: liveDisplayOffset[0],
          offsetY: liveDisplayOffset[1],
          fit,
          imageSizeDisplayPx,
          hoverIndex: interaction.hoverIndex,
          showLabels: interaction.zoom >= 0.7,
          dim: interaction.dim,
          showResizeHandles: imageSizePx !== null,
        })
      : null,
  );

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
    void layout;
    void liveDisplayOffset;
    void imageSizeDisplayPx;
    void interaction.hoverIndex;
    void interaction.dim;
    void interaction.dragMode;
    void interaction.dragLiveDelta;
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
      imageSizeDisplayPx,
      hoverIndex: interaction.hoverIndex,
      showLabels: interaction.zoom >= 0.7,
      dim: interaction.dim,
      showResizeHandles: imageSizePx !== null,
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

  function onPointerMoveCanvas(ev: PointerEvent) {
    if (!canvasEl) return;
    interaction.onPointerMove(ev, canvasEl);
    if (!interaction.dragging) {
      const rect = canvasEl.getBoundingClientRect();
      hoverCorner = interaction.hitResizeHandle(ev.clientX - rect.left, ev.clientY - rect.top);
    }
  }

  // "Cover all monitors": the smallest aspect-preserving image rect that
  // fully covers the canvas (union of monitor rects + bezels). Saves the
  // user from manually scaling for the portrait + landscape case.
  function snapToCover() {
    if (!image || image.naturalWidth <= 0 || image.naturalHeight <= 0) return;
    const coreCanvasW = layout.totalMmW * layout.coreMmToPx;
    const coreCanvasH = layout.totalMmH * layout.coreMmToPx;
    if (coreCanvasW <= 0 || coreCanvasH <= 0) return;
    const aspect = image.naturalWidth / image.naturalHeight;
    let w: number;
    let h: number;
    if (coreCanvasW / aspect >= coreCanvasH) {
      w = coreCanvasW;
      h = coreCanvasW / aspect;
    } else {
      w = coreCanvasH * aspect;
      h = coreCanvasH;
    }
    const offX = (coreCanvasW - w) / 2;
    const offY = (coreCanvasH - h) / 2;
    onTransformCommit?.(
      [Math.round(offX), Math.round(offY)],
      [Math.max(1, Math.round(w)), Math.max(1, Math.round(h))],
    );
  }

  const cursorStyle = $derived.by(() => {
    if (interaction.dragging) {
      return interaction.dragMode?.kind === 'resize'
        ? (interaction.cursorForResize(interaction.dragMode.corner) ?? 'grabbing')
        : 'grabbing';
    }
    if (hoverCorner) {
      return interaction.cursorForResize(hoverCorner) ?? 'grab';
    }
    return interaction.hoverIndex !== null ? 'pointer' : 'grab';
  });

  const popoutRect = $derived<MonitorRect | null>(interaction.popoutRect());

  function eventToCanvas(ev: DragEvent): [number, number] | null {
    if (!canvasEl) return null;
    const rect = canvasEl.getBoundingClientRect();
    return [ev.clientX - rect.left, ev.clientY - rect.top];
  }

  function dragHasImagePayload(ev: DragEvent): boolean {
    const dt = ev.dataTransfer;
    if (!dt) return false;
    return Array.from(dt.types).some(
      (t) => t === 'application/x-superpanels-image' || t === 'text/uri-list',
    );
  }

  function onDragOver(ev: DragEvent) {
    if (!dragHasImagePayload(ev)) return;
    ev.preventDefault();
    if (ev.dataTransfer) ev.dataTransfer.dropEffect = 'copy';
    const xy = eventToCanvas(ev);
    if (!xy) return;
    dropHoverIdx = hitTest(layout, xy[0], xy[1]);
  }

  function onDragLeave() {
    dropHoverIdx = null;
  }

  function onDrop(ev: DragEvent) {
    if (!dragHasImagePayload(ev)) return;
    ev.preventDefault();
    const dt = ev.dataTransfer;
    if (!dt) return;
    const path =
      dt.getData('application/x-superpanels-image') ||
      dt
        .getData('text/uri-list')
        .replace(/^file:\/\//, '')
        .split(/\r?\n/)[0]
        ?.trim();
    if (!path) return;
    const xy = eventToCanvas(ev);
    if (!xy) return;
    const idx = hitTest(layout, xy[0], xy[1]);
    dropHoverIdx = null;
    if (idx === null) return;
    onMonitorDrop?.(idx, path);
  }
</script>

<!-- reason: dragover/drop are the spec interaction (Phase 4b §4b.1 — drop
     image onto monitor); the wrapper hosts the canvas which is keyboard-
     reachable via tab + R/Esc shortcuts on `interaction.onKey`. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={wrapperEl}
  class="relative h-full w-full overflow-hidden rounded border border-slate-800 bg-bezel"
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
>
  <canvas
    bind:this={canvasEl}
    class="absolute inset-0 h-full w-full select-none"
    style="touch-action: none;"
    style:cursor={cursorStyle}
    onpointermove={onPointerMoveCanvas}
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
    class="pointer-events-none absolute right-2 top-2 flex flex-col items-end gap-1 text-[10px] uppercase tracking-wide text-slate-400"
  >
    <span class="rounded bg-slate-900/70 px-1.5 py-0.5">
      zoom {(interaction.zoom * 100).toFixed(0)}%
    </span>
    <span class="rounded bg-slate-900/70 px-1.5 py-0.5">
      dim {interaction.dim ? 'on' : 'off'} · D
    </span>
  </div>

  {#if dropHoverIdx !== null}
    {@const m = layout.monitors.find((r) => r.monitorIndex === dropHoverIdx)}
    {#if m}
      <div
        class="pointer-events-none absolute rounded border-2 border-accent/80 bg-accent/15"
        style:left={`${m.x}px`}
        style:top={`${m.y}px`}
        style:width={`${m.w}px`}
        style:height={`${m.h}px`}
      ></div>
    {/if}
  {/if}

  {#if image && imagePath}
    <div class="absolute left-2 top-2 flex gap-1 text-[10px] uppercase text-slate-200">
      <button
        type="button"
        class="rounded bg-slate-900/80 px-2 py-1 hover:bg-slate-800"
        title="Snap the image to cover all monitors at the smallest aspect-preserving scale."
        onclick={snapToCover}
      >
        Cover all monitors
      </button>
      <button
        type="button"
        class="rounded px-2 py-1 hover:bg-slate-800"
        class:bg-slate-900={!aspectLock}
        class:bg-accent={aspectLock}
        class:text-slate-900={aspectLock}
        title={aspectLock
          ? 'Aspect lock on — corner resize keeps the natural image ratio.'
          : 'Aspect lock off — corner resize stretches each axis independently.'}
        onclick={() => (aspectLock = !aspectLock)}
      >
        Lock aspect{aspectLock ? ' · on' : ' · off'}
      </button>
    </div>
  {/if}

  {#if popoutRect}
    <MonitorPopout
      {layout}
      rect={popoutRect}
      {image}
      {fit}
      offset={liveDisplayOffset}
      {imageSizeDisplayPx}
      onClose={() => interaction.closePopout()}
    />
  {/if}
</div>
