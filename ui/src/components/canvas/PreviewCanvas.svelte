<!-- reason: one coherent rendering + hit-test surface. Projection geometry is
     shared by drawing and unified pointer hit-testing across monitors, the span
     image, and composite layers; splitting hit-test from projection would
     fragment a single concern. Visual sub-surfaces live in Canvas*.svelte. -->
<script lang="ts">
  // Bezel-aware monitor preview canvas. Free-positioning model: source images
  // float in mm-space and monitors are crop windows hovering over them. Single
  // image (span) or several (composite) — unified hit-testing routes drags to a
  // monitor (rearrange), an image/layer (pan), a resize handle, or the stage.

  import { onMount } from 'svelte';
  import type { Monitor } from '$lib/api';
  import { canvasView } from '$lib/stores/canvas-view.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import {
    bbox,
    buildPreviewMonitors,
    hNeighbourPairs,
    vNeighbourPairs,
    type PreviewMonitor,
  } from '$lib/canvas/preview-layout';
  import { createDragController } from '$lib/canvas/drag.svelte';
  import { cursorFor, hitTest, type Hit } from '$lib/canvas/hit-test';
  import type { ImageTransform } from '$lib/stores/image-transform.svelte';
  import type { CanvasLayer } from '$lib/stores/canvas-layers.svelte';
  import CanvasGrid from './CanvasGrid.svelte';
  import CanvasMonitors from './CanvasMonitors.svelte';
  import CanvasSpanImage from './CanvasSpanImage.svelte';
  import CanvasImageLayers from './CanvasImageLayers.svelte';
  import DimensionLines from './DimensionLines.svelte';

  type Props = {
    monitors: Monitor[];
    bezelHmm: number;
    imageUrl: string | null;
    imageTransform: ImageTransform;
    onImageTransformChange: (next: ImageTransform) => void;
    onMonitorDrop?: (monitorId: string, path: string) => void;
    // Composite mode — non-empty `layers` switches the canvas from the single
    // span image to a stack of independently-draggable, removable layers.
    layers?: CanvasLayer[];
    onLayerTransformChange?: (id: string, next: ImageTransform) => void;
    onLayerRemove?: (id: string) => void;
    onLayerSelect?: (id: string) => void;
    onLayerSnap?: (id: string, axis: 'width' | 'height') => void;
  };

  let {
    monitors,
    bezelHmm,
    imageUrl,
    imageTransform,
    onImageTransformChange,
    onMonitorDrop,
    layers = [],
    onLayerTransformChange,
    onLayerRemove,
    onLayerSelect,
    onLayerSnap,
  }: Props = $props();

  const compositeMode = $derived(layers.length > 0);
  const imagesInteractive = $derived(canvasView.mode === 'images');

  // The on-image button cluster needs room; hide it (and its hit regions) on
  // layers rendered below this width. The ToolDock mirrors the actions.
  const MIN_BUTTONS_PX = 96;
  function layerShowsButtons(id: string): boolean {
    const entry = layerRects.find((l) => l.layer.id === id);
    return entry ? entry.rect.w >= MIN_BUTTONS_PX && entry.rect.h >= 44 : false;
  }

  let stageEl: HTMLDivElement | undefined = $state();
  let stageW = $state(1200);
  let stageH = $state(700);

  let tip = $state<{ x: number; y: number; m: PreviewMonitor } | null>(null);
  let dropHover = $state<string | null>(null);
  let hoverLayerId = $state<string | null>(null);
  // Click-vs-drag disambiguation: a monitor is selected (which surfaces the
  // inspector) only on a discrete click-release. Hold-and-drag must not flash
  // the inspector open mid-gesture.
  let pendingSelect: { id: string; sx: number; sy: number } | null = null;
  const CLICK_SLOP_PX = 4;

  const previewMonitors = $derived(buildPreviewMonitors(monitors, canvasView.overrides));

  const layout = $derived.by(() => {
    const bb = bbox(previewMonitors);
    const pad = 100;
    const totalW = bb.w + pad * 2;
    const totalH = bb.h + pad * 2;
    const sx = stageW / totalW;
    const sy = stageH / totalH;
    const baseScale = Math.min(sx, sy) * 0.86;
    const scale = baseScale * canvasView.zoom;
    const cx = stageW / 2 + canvasView.panX;
    const cy = stageH / 2 + canvasView.panY;
    return {
      bb,
      scale,
      ox: cx - (bb.x + bb.w / 2) * scale,
      oy: cy - (bb.y + bb.h / 2) * scale,
    };
  });

  function mm2px(mmX: number, mmY: number): { x: number; y: number } {
    return { x: layout.ox + mmX * layout.scale, y: layout.oy + mmY * layout.scale };
  }

  function rectOf(t: ImageTransform): { x: number; y: number; w: number; h: number } {
    const a = mm2px(t.offsetMmX, t.offsetMmY);
    return { x: a.x, y: a.y, w: t.widthMm * layout.scale, h: t.heightMm * layout.scale };
  }

  const imgRect = $derived(rectOf(imageTransform));
  const layerRects = $derived(layers.map((l) => ({ layer: l, rect: rectOf(l.transform) })));

  const monitorRects = $derived(
    previewMonitors.map((m) => {
      const a = mm2px(m.xMm, m.yMm);
      const b = mm2px(m.xMm + m.wMm, m.yMm + m.hMm);
      return { x: a.x, y: a.y, w: b.x - a.x, h: b.y - a.y };
    }),
  );

  const renderedMonitors = $derived(
    previewMonitors.map((m, i) => {
      const r = monitorRects[i] ?? { x: 0, y: 0, w: 0, h: 0 };
      return {
        id: m.id,
        name: m.name,
        pxW: m.pxW,
        pxH: m.pxH,
        missing: m.missing,
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
        isSel: canvasView.selectId === m.id,
        isHover: canvasView.hoverId === m.id || dropHover === m.id,
      };
    }),
  );

  const renderedLayers = $derived(
    layerRects.map(({ layer, rect }) => ({
      id: layer.id,
      url: layer.url,
      x: rect.x,
      y: rect.y,
      w: rect.w,
      h: rect.h,
      // Dim layers while editing monitors so they read as a backdrop.
      dimmed: canvasView.dim || !imagesInteractive,
      selected: canvasView.selectedLayerId === layer.id,
      hovered: imagesInteractive && hoverLayerId === layer.id,
      showButtons: rect.w >= MIN_BUTTONS_PX && rect.h >= 44,
    })),
  );

  const dragController = createDragController({
    monitors: () => previewMonitors,
    imageTransform: () => imageTransform,
    scale: () => layout.scale,
    bezelHmm: () => bezelHmm,
    setImageTransform: (t) => onImageTransformChange(t),
    getLayer: (id) => layers.find((l) => l.id === id)?.transform ?? null,
    setLayerTransform: (id, t) => onLayerTransformChange?.(id, t),
  });

  function hitAt(clientX: number, clientY: number): Hit {
    if (!stageEl) return { type: 'stage' };
    const r = stageEl.getBoundingClientRect();
    return hitTest(clientX - r.left, clientY - r.top, {
      compositeMode,
      imagesInteractive,
      buttonsForLayer: layerShowsButtons,
      layerRects: layerRects.map(({ layer, rect }) => ({ id: layer.id, rect })),
      monitors: previewMonitors.map((m, i) => ({
        id: m.id,
        rect: monitorRects[i] ?? { x: 0, y: 0, w: 0, h: 0 },
      })),
      imageUrl,
      imgRect,
    });
  }

  function onPointerDown(ev: PointerEvent) {
    if (ev.button !== 0) return;
    const hit = hitAt(ev.clientX, ev.clientY);
    if (hit.type === 'layer-remove') {
      onLayerRemove?.(hit.id);
      return;
    } else if (hit.type === 'layer-snap') {
      canvasView.setSelectedLayerId(hit.id);
      onLayerSnap?.(hit.id, hit.axis);
      return;
    } else if (hit.type === 'layer') {
      onLayerSelect?.(hit.id);
      canvasView.setSelectedLayerId(hit.id);
      const l = layers.find((x) => x.id === hit.id);
      if (l)
        dragController.begin({
          kind: 'layer-image',
          id: hit.id,
          startX: ev.clientX,
          startY: ev.clientY,
          startMmX: l.transform.offsetMmX,
          startMmY: l.transform.offsetMmY,
        });
    } else if (hit.type === 'layer-resize') {
      onLayerSelect?.(hit.id);
      canvasView.setSelectedLayerId(hit.id);
      const l = layers.find((x) => x.id === hit.id);
      if (l)
        dragController.begin({
          kind: 'layer-resize',
          id: hit.id,
          startX: ev.clientX,
          startW: l.transform.widthMm,
          startH: l.transform.heightMm,
          aspect: l.transform.widthMm / l.transform.heightMm,
        });
    } else if (hit.type === 'monitor') {
      const m = previewMonitors.find((x) => x.id === hit.id);
      if (!m) return;
      pendingSelect =
        canvasView.selectId === hit.id ? null : { id: hit.id, sx: ev.clientX, sy: ev.clientY };
      dragController.begin({
        kind: 'monitor',
        id: hit.id,
        startX: ev.clientX,
        startY: ev.clientY,
        startMmX: m.xMm,
        startMmY: m.yMm,
      });
    } else if (hit.type === 'image') {
      dragController.begin({
        kind: 'image',
        startX: ev.clientX,
        startY: ev.clientY,
        startMmX: imageTransform.offsetMmX,
        startMmY: imageTransform.offsetMmY,
      });
    } else if (hit.type === 'image-resize') {
      dragController.begin({
        kind: 'image-resize',
        startX: ev.clientX,
        startW: imageTransform.widthMm,
        startH: imageTransform.heightMm,
        aspect: imageTransform.widthMm / imageTransform.heightMm,
      });
    } else {
      canvasView.setSelectId(null);
      canvasView.setSelectedLayerId(null);
      dragController.begin({
        kind: 'pan',
        startX: ev.clientX,
        startY: ev.clientY,
        startOx: canvasView.panX,
        startOy: canvasView.panY,
      });
    }
    if (stageEl) stageEl.setPointerCapture(ev.pointerId);
  }

  function onPointerMove(ev: PointerEvent) {
    if (!stageEl) return;
    const r = stageEl.getBoundingClientRect();
    const px = ev.clientX - r.left;
    const py = ev.clientY - r.top;

    if (!dragController.drag) {
      const hit = hitAt(ev.clientX, ev.clientY);
      hoverLayerId =
        hit.type === 'layer' || hit.type === 'layer-resize' || hit.type === 'layer-remove'
          ? hit.id
          : null;
      if (hit.type === 'monitor') {
        const m = previewMonitors.find((x) => x.id === hit.id) ?? null;
        canvasView.setHoverId(hit.id);
        tip = m ? { x: px + 14, y: py + 14, m } : null;
      } else {
        canvasView.setHoverId(null);
        tip = null;
      }
      stageEl.style.cursor = cursorFor(hit);
      return;
    }
    if (
      pendingSelect &&
      Math.hypot(ev.clientX - pendingSelect.sx, ev.clientY - pendingSelect.sy) > CLICK_SLOP_PX
    ) {
      pendingSelect = null;
    }
    dragController.move(ev);
  }

  function onPointerUp(ev: PointerEvent) {
    if (pendingSelect) {
      canvasView.setSelectId(pendingSelect.id);
      pendingSelect = null;
    }
    dragController.end();
    if (stageEl) stageEl.releasePointerCapture(ev.pointerId);
  }

  function onWheel(ev: WheelEvent) {
    ev.preventDefault();
    canvasView.setZoom(canvasView.zoom * (1 + -ev.deltaY * 0.001));
  }

  const dimLines = $derived.by(() => {
    const showAlways = ui.dimsAlways;
    if (!showAlways && !dragController.drag && !canvasView.hoverId && !canvasView.selectId)
      return [];
    const out: Array<{ x1: number; y1: number; x2: number; y2: number; label: string }> = [];
    for (const p of hNeighbourPairs(previewMonitors)) {
      if (p.gapMm <= 0.1) continue;
      const yMm = Math.max(p.a.yMm, p.b.yMm) + Math.min(p.a.hMm, p.b.hMm) / 2;
      const p1 = mm2px(p.a.xMm + p.a.wMm, yMm);
      const p2 = mm2px(p.b.xMm, yMm);
      out.push({ x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y, label: `${Math.round(p.gapMm)} mm` });
    }
    for (const p of vNeighbourPairs(previewMonitors)) {
      if (p.gapMm <= 0.1) continue;
      const xMm = Math.max(p.a.xMm, p.b.xMm) + Math.min(p.a.wMm, p.b.wMm) / 2;
      const p1 = mm2px(xMm, p.a.yMm + p.a.hMm);
      const p2 = mm2px(xMm, p.b.yMm);
      out.push({ x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y, label: `${Math.round(p.gapMm)} mm` });
    }
    return out;
  });

  const isFlashing = $derived(Boolean(runtime.flashAt && Date.now() - runtime.flashAt < 500));

  onMount(() => {
    if (!stageEl) return;
    const ro = new ResizeObserver(([entry]) => {
      if (!entry) return;
      stageW = entry.contentRect.width;
      stageH = entry.contentRect.height;
    });
    ro.observe(stageEl);
    return () => ro.disconnect();
  });

  function handleDrop(ev: DragEvent) {
    ev.preventDefault();
    dropHover = null;
    const path =
      ev.dataTransfer?.getData('application/x-superpanels-image') ??
      ev.dataTransfer?.getData('text/plain') ??
      '';
    if (!path) return;
    const hit = hitAt(ev.clientX, ev.clientY);
    if (hit.type === 'monitor') onMonitorDrop?.(hit.id, path);
  }

  function handleDragOver(ev: DragEvent) {
    if (!ev.dataTransfer) return;
    ev.preventDefault();
    const hit = hitAt(ev.clientX, ev.clientY);
    dropHover = hit.type === 'monitor' ? hit.id : null;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={stageEl}
  class="absolute inset-0 select-none overflow-hidden"
  style:background="radial-gradient(ellipse at 50% 40%, var(--bg-2), var(--bg) 70%), var(--bg)"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  onpointerleave={() => {
    canvasView.setHoverId(null);
    hoverLayerId = null;
    tip = null;
  }}
  onwheel={onWheel}
  ondrop={handleDrop}
  ondragover={handleDragOver}
  ondragleave={() => (dropHover = null)}
>
  <CanvasGrid scale={layout.scale} ox={layout.ox} oy={layout.oy} />

  {#if compositeMode}
    <CanvasImageLayers layers={renderedLayers} dragging={Boolean(dragController.drag)} />
  {:else if imageUrl}
    <CanvasSpanImage
      {imageUrl}
      rect={imgRect}
      dim={canvasView.dim}
      dragging={Boolean(dragController.drag)}
      {monitorRects}
    />
  {/if}

  <CanvasMonitors monitors={renderedMonitors} flashing={isFlashing} />

  <DimensionLines lines={dimLines} />

  {#each dragController.guides as g, i (i)}
    {#if g.kind === 'h'}
      {@const p = mm2px(0, g.y)}
      <div
        class="pointer-events-none absolute"
        style:left="0"
        style:right="0"
        style:top="{p.y}px"
        style:height="1px"
        style:background="var(--accent)"
        style:opacity="0.7"
      ></div>
    {:else}
      {@const p = mm2px(g.x, 0)}
      <div
        class="pointer-events-none absolute"
        style:top="0"
        style:bottom="0"
        style:left="{p.x}px"
        style:width="1px"
        style:background="var(--accent)"
        style:opacity="0.7"
      ></div>
    {/if}
  {/each}

  {#if tip}
    <div class="tip" style:left="{tip.x}px" style:top="{tip.y}px">
      <div style:font-weight="600" style:font-size="12px">{tip.m.name}</div>
      <div class="mono" style:color="var(--text-2)" style:margin-top="4px">
        {tip.m.pxW}×{tip.m.pxH}{tip.m.refreshHz ? ` @ ${tip.m.refreshHz}Hz` : ''}
      </div>
      <div class="mono" style:color="var(--text-3)" style:margin-top="2px">
        {Math.round(tip.m.wMm)}×{Math.round(tip.m.hMm)} mm
      </div>
    </div>
  {/if}
</div>
