<!-- reason: coherent rendering+hit-test surface; geometry is shared by draw and hit-test, further splitting would fragment a single concern -->
<script lang="ts">
  // Bezel-aware monitor preview canvas. Free-positioning model: the source
  // image floats in mm-space and monitors are crop windows that hover over
  // it. Unified hit-testing — drag a monitor to rearrange (preview override),
  // drag the image to pan, drag the empty stage to pan the view.

  import { onMount } from 'svelte';
  import type { Monitor } from '$lib/api';
  import { canvasView } from '$lib/stores/canvasView.svelte';
  import { runtime } from '$lib/stores/runtime.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import {
    bbox,
    buildPreviewMonitors,
    hNeighbourPairs,
    vNeighbourPairs,
    type PreviewMonitor,
  } from '$lib/canvas/previewLayout';
  import { createDragController } from '$lib/canvas/drag.svelte';
  import CanvasGrid from './CanvasGrid.svelte';
  import DimensionLines from './DimensionLines.svelte';

  type ImageTransform = {
    offsetMmX: number;
    offsetMmY: number;
    widthMm: number;
    heightMm: number;
  };

  type Props = {
    monitors: Monitor[];
    bezelHmm: number;
    imageUrl: string | null;
    imageTransform: ImageTransform;
    onImageTransformChange: (next: ImageTransform) => void;
    onMonitorDrop?: (monitorId: string, path: string) => void;
  };

  let {
    monitors,
    bezelHmm,
    imageUrl,
    imageTransform,
    onImageTransformChange,
    onMonitorDrop,
  }: Props = $props();

  let stageEl: HTMLDivElement | undefined = $state();
  let stageW = $state(1200);
  let stageH = $state(700);

  let tip = $state<{ x: number; y: number; m: PreviewMonitor } | null>(null);
  let dropHover = $state<string | null>(null);

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

  const imgRect = $derived.by(() => {
    const a = mm2px(imageTransform.offsetMmX, imageTransform.offsetMmY);
    return {
      x: a.x,
      y: a.y,
      w: imageTransform.widthMm * layout.scale,
      h: imageTransform.heightMm * layout.scale,
    };
  });

  const dragController = createDragController({
    monitors: () => previewMonitors,
    imageTransform: () => imageTransform,
    scale: () => layout.scale,
    bezelHmm: () => bezelHmm,
    setImageTransform: (t) => onImageTransformChange(t),
  });

  type Hit =
    | { type: 'monitor'; id: string }
    | { type: 'rotate'; id: string }
    | { type: 'image' }
    | { type: 'image-resize' }
    | { type: 'stage' };

  function hitTest(clientX: number, clientY: number): Hit {
    if (!stageEl) return { type: 'stage' };
    const r = stageEl.getBoundingClientRect();
    const px = clientX - r.left;
    const py = clientY - r.top;
    if (canvasView.selectId) {
      const sel = previewMonitors.find((m) => m.id === canvasView.selectId);
      if (sel) {
        const b = mm2px(sel.xMm + sel.wMm, sel.yMm);
        const cx = b.x - 4;
        const cy = b.y - 13;
        if (Math.hypot(px - cx, py - cy) < 12) return { type: 'rotate', id: sel.id };
      }
    }
    for (let i = previewMonitors.length - 1; i >= 0; i -= 1) {
      const m = previewMonitors[i];
      if (!m) continue;
      const a = mm2px(m.xMm, m.yMm);
      const b = mm2px(m.xMm + m.wMm, m.yMm + m.hMm);
      if (px >= a.x && px <= b.x && py >= a.y && py <= b.y) {
        return { type: 'monitor', id: m.id };
      }
    }
    if (imageUrl) {
      if (Math.hypot(imgRect.x + imgRect.w - px, imgRect.y + imgRect.h - py) < 24)
        return { type: 'image-resize' };
      if (
        px >= imgRect.x &&
        px <= imgRect.x + imgRect.w &&
        py >= imgRect.y &&
        py <= imgRect.y + imgRect.h
      ) {
        return { type: 'image' };
      }
    }
    return { type: 'stage' };
  }

  function rotateMonitor(id: string, delta: number) {
    const cur = canvasView.overrides[id];
    if (!cur) return;
    const next = (((cur.rotation + delta) % 360) + 360) % 360;
    canvasView.override(id, { rotation: next as 0 | 90 | 180 | 270 });
  }

  function onPointerDown(ev: PointerEvent) {
    if (ev.button !== 0) return;
    const hit = hitTest(ev.clientX, ev.clientY);
    if (hit.type === 'rotate') {
      rotateMonitor(hit.id, 90);
      return;
    }
    if (hit.type === 'monitor') {
      const m = previewMonitors.find((x) => x.id === hit.id);
      if (!m) return;
      canvasView.setSelectId(hit.id);
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
      const hit = hitTest(ev.clientX, ev.clientY);
      if (hit.type === 'rotate') {
        canvasView.setHoverId(null);
        tip = null;
        stageEl.style.cursor = 'pointer';
      } else if (hit.type === 'monitor') {
        const m = previewMonitors.find((x) => x.id === hit.id) ?? null;
        canvasView.setHoverId(hit.id);
        tip = m ? { x: px + 14, y: py + 14, m } : null;
        stageEl.style.cursor = 'grab';
      } else if (hit.type === 'image') {
        canvasView.setHoverId(null);
        tip = null;
        stageEl.style.cursor = 'move';
      } else if (hit.type === 'image-resize') {
        canvasView.setHoverId(null);
        tip = null;
        stageEl.style.cursor = 'nwse-resize';
      } else {
        canvasView.setHoverId(null);
        tip = null;
        stageEl.style.cursor = 'default';
      }
      return;
    }
    dragController.move(ev);
  }

  function onPointerUp(ev: PointerEvent) {
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

  const isFlashing = $derived(runtime.flashAt && Date.now() - runtime.flashAt < 500);

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
    const hit = hitTest(ev.clientX, ev.clientY);
    if (hit.type === 'monitor') onMonitorDrop?.(hit.id, path);
  }

  function handleDragOver(ev: DragEvent) {
    if (!ev.dataTransfer) return;
    ev.preventDefault();
    const hit = hitTest(ev.clientX, ev.clientY);
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
    tip = null;
  }}
  onwheel={onWheel}
  ondrop={handleDrop}
  ondragover={handleDragOver}
  ondragleave={() => (dropHover = null)}
>
  <CanvasGrid scale={layout.scale} ox={layout.ox} oy={layout.oy} />

  {#if imageUrl}
    <div
      class="pointer-events-none absolute"
      style:left="{imgRect.x}px"
      style:top="{imgRect.y}px"
      style:width="{imgRect.w}px"
      style:height="{imgRect.h}px"
      style:background-image="url({imageUrl})"
      style:background-size="cover"
      style:background-position="center"
      style:opacity={canvasView.dim ? '0.18' : '1'}
      style:transition={dragController.drag ? 'none' : 'opacity 200ms ease'}
      style:box-shadow={canvasView.dim ? 'none' : '0 0 0 1px oklch(1 0 0 / 0.06)'}
    ></div>

    {#if canvasView.dim}
      {#each previewMonitors as m (m.id)}
        {@const a = mm2px(m.xMm, m.yMm)}
        {@const b = mm2px(m.xMm + m.wMm, m.yMm + m.hMm)}
        <div
          class="pointer-events-none absolute"
          style:left="{a.x}px"
          style:top="{a.y}px"
          style:width="{b.x - a.x}px"
          style:height="{b.y - a.y}px"
          style:background-image="url({imageUrl})"
          style:background-size="{imgRect.w}px {imgRect.h}px"
          style:background-position="{imgRect.x - a.x}px {imgRect.y - a.y}px"
          style:background-repeat="no-repeat"
        ></div>
      {/each}
    {/if}

    <div
      class="pointer-events-none absolute rounded-sm"
      style:left="{imgRect.x + imgRect.w - 6}px"
      style:top="{imgRect.y + imgRect.h - 6}px"
      style:width="12px"
      style:height="12px"
      style:background="var(--accent)"
      style:border="2px solid var(--bg)"
      style:opacity="0.85"
    ></div>
  {/if}

  {#each previewMonitors as m (m.id)}
    {@const a = mm2px(m.xMm, m.yMm)}
    {@const b = mm2px(m.xMm + m.wMm, m.yMm + m.hMm)}
    {@const isSel = canvasView.selectId === m.id}
    {@const isHover = canvasView.hoverId === m.id || dropHover === m.id}
    <div
      class="pointer-events-none absolute"
      style:left="{a.x}px"
      style:top="{a.y}px"
      style:width="{b.x - a.x}px"
      style:height="{b.y - a.y}px"
      style:border="1.5px solid {isSel
        ? 'var(--accent)'
        : isHover
          ? 'var(--text-2)'
          : 'var(--line-2)'}"
      style:border-radius="3px"
      style:transition="border-color 80ms, box-shadow 80ms"
      style:box-shadow={isSel
        ? '0 0 0 1px color-mix(in oklab, var(--accent) 30%, transparent), 0 0 24px color-mix(in oklab, var(--accent) 25%, transparent)'
        : isHover
          ? '0 0 12px oklch(1 0 0 / 0.15)'
          : 'none'}
      style:animation={isFlashing ? 'applyFlash 380ms ease-out' : 'none'}
    >
      <div
        class="pointer-events-none absolute"
        style:inset="4px"
        style:border="1px solid oklch(0 0 0 / 0.4)"
        style:border-radius="1px"
      ></div>
      <div
        class="pointer-events-none mono absolute font-semibold"
        style:top="6px"
        style:left="8px"
        style:font-size="10px"
        style:letter-spacing="0.04em"
        style:color={isSel ? 'var(--accent)' : 'var(--text-2)'}
        style:text-shadow="0 1px 2px oklch(0 0 0 / 0.6)"
      >
        {m.name}{m.primary ? ' ★' : ''}
      </div>
      <div
        class="pointer-events-none mono absolute"
        style:bottom="6px"
        style:right="8px"
        style:font-size="9px"
        style:color="var(--text-3)"
        style:text-shadow="0 1px 2px oklch(0 0 0 / 0.6)"
      >
        {m.pxW}×{m.pxH}
      </div>
      {#if m.missing}
        <div
          class="pointer-events-none absolute mono"
          style:top="6px"
          style:right="8px"
          style:font-size="9px"
          style:color="var(--warn)"
          style:text-shadow="0 1px 2px oklch(0 0 0 / 0.6)"
        >
          mm?
        </div>
      {/if}
    </div>
    {#if isSel}
      <div
        class="pointer-events-none absolute"
        style:left="{b.x - 4 - 18}px"
        style:top="{a.y - 22}px"
        style:width="18px"
        style:height="18px"
        style:border-radius="50%"
        style:background="var(--accent)"
        style:display="flex"
        style:align-items="center"
        style:justify-content="center"
        style:color="oklch(0.16 0.01 250)"
        style:font-size="11px"
        style:font-weight="700"
        style:box-shadow="0 2px 6px oklch(0 0 0 / 0.4)"
        title="Rotate 90°"
      >
        ↻
      </div>
    {/if}
  {/each}

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
