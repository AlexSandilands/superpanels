<script lang="ts">
  // Side popout showing details + thumbnail crop slice for a clicked monitor.
  // The slice rectangle is computed client-side from the canvas layout so we
  // don't pay an IPC roundtrip per click; the *applied* crop runs through
  // `compute_crop_specs` in core (SPEC §4.4).

  import type { CanvasLayout, MonitorRect } from '$lib/canvas/types';

  type Props = {
    layout: CanvasLayout;
    rect: MonitorRect;
    image: HTMLImageElement | null;
    fit: 'fill' | 'fit' | 'stretch' | 'center';
    offset: [number, number];
    /** Display-px image rectangle when free-positioning is active. */
    imageSizeDisplayPx: [number, number] | null;
    onClose: () => void;
  };
  let { layout, rect, image, fit, offset, imageSizeDisplayPx, onClose }: Props = $props();

  const POPOUT_W = 240;
  const POPOUT_H = 200;
  const padding = 12;

  // Compute the source-image rectangle that would be drawn in the monitor's
  // canvas window. With free-positioning (`imageSizeDisplayPx` set), the image
  // rectangle on the canvas is `(offset, imageSizeDisplayPx)` in display px
  // and we map straight from there. Without it we fall back to the FitMode
  // placement so saved-without-image_size_px profiles still preview.
  const slice = $derived.by(() => {
    if (!image || image.naturalWidth <= 0 || image.naturalHeight <= 0) return null;
    let imgRectW: number;
    let imgRectH: number;
    let imgRectX: number;
    let imgRectY: number;
    if (imageSizeDisplayPx) {
      [imgRectW, imgRectH] = imageSizeDisplayPx;
      imgRectX = layout.offsetX + offset[0];
      imgRectY = layout.offsetY + offset[1];
    } else {
      const canvasW = layout.totalW;
      const canvasH = layout.totalH;
      if (canvasW <= 0 || canvasH <= 0) return null;
      const placement = placeImage(fit, canvasW, canvasH, image.naturalWidth, image.naturalHeight);
      imgRectW = placement.w;
      imgRectH = placement.h;
      imgRectX = layout.offsetX + placement.x + offset[0];
      imgRectY = layout.offsetY + placement.y + offset[1];
    }
    if (imgRectW <= 0 || imgRectH <= 0) return null;
    const srcPerPxX = image.naturalWidth / imgRectW;
    const srcPerPxY = image.naturalHeight / imgRectH;
    const srcX = (rect.x - imgRectX) * srcPerPxX;
    const srcY = (rect.y - imgRectY) * srcPerPxY;
    const srcW = rect.w * srcPerPxX;
    const srcH = rect.h * srcPerPxY;
    return { x: srcX, y: srcY, w: srcW, h: srcH };
  });

  type Placement = { x: number; y: number; w: number; h: number };
  function placeImage(
    f: 'fill' | 'fit' | 'stretch' | 'center',
    cw: number,
    ch: number,
    iw: number,
    ih: number,
  ): Placement {
    if (f === 'stretch') return { x: 0, y: 0, w: cw, h: ch };
    if (f === 'center') return { x: (cw - iw) / 2, y: (ch - ih) / 2, w: iw, h: ih };
    const cr = cw / ch;
    const ir = iw / ih;
    const cover = f === 'fill';
    const useW = cover ? ir < cr : ir > cr;
    const w = useW ? cw : ch * ir;
    const h = useW ? cw / ir : ch;
    return { x: (cw - w) / 2, y: (ch - h) / 2, w, h };
  }

  // Position the popout to the right of the monitor unless that overflows.
  const position = $derived.by(() => {
    const right = rect.x + rect.w + padding;
    const overflowsRight = right + POPOUT_W > layout.offsetX + layout.totalW + 200;
    const x = overflowsRight ? Math.max(8, rect.x - POPOUT_W - padding) : right;
    const y = Math.max(8, rect.y);
    return { x, y };
  });

  // CSS background-position/size that displays the slice as a visible thumbnail.
  const previewStyle = $derived.by(() => {
    if (!image || !slice) return '';
    const previewW = POPOUT_W - 16;
    const previewH = 110;
    const scaleX = previewW / Math.max(1, slice.w);
    const scaleY = previewH / Math.max(1, slice.h);
    const scale = Math.min(scaleX, scaleY);
    const bgW = image.naturalWidth * scale;
    const bgH = image.naturalHeight * scale;
    const bgX = -slice.x * scale;
    const bgY = -slice.y * scale;
    return `background-image: url(${image.src}); background-size: ${bgW}px ${bgH}px; background-position: ${bgX}px ${bgY}px;`;
  });
</script>

<div
  class="absolute z-20 rounded border border-slate-700 bg-slate-900/95 p-3 text-xs text-slate-200 shadow-lg"
  style:left="{position.x}px"
  style:top="{position.y}px"
  style:width="{POPOUT_W}px"
  style:min-height="{POPOUT_H}px"
>
  <header class="mb-2 flex items-center justify-between">
    <span class="font-mono font-semibold">{rect.monitorName}</span>
    <button
      type="button"
      class="rounded border border-slate-700 px-1.5 py-0.5 text-[10px] hover:bg-slate-800"
      onclick={onClose}
      aria-label="Close monitor popout"
    >
      ✕
    </button>
  </header>

  <dl class="mb-2 grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-[11px] text-slate-400">
    <dt>Resolution</dt>
    <dd class="font-mono text-slate-200">{rect.pixelW}×{rect.pixelH}</dd>
    <dt>Physical</dt>
    <dd class="font-mono {rect.missing ? 'text-amber-400' : 'text-slate-200'}">
      {rect.missing ? 'unknown' : `${Math.round(rect.widthMm)}×${Math.round(rect.heightMm)} mm`}
    </dd>
    {#if rect.rotated}
      <dt>Rotation</dt>
      <dd class="text-slate-200">portrait</dd>
    {/if}
  </dl>

  {#if image && slice}
    <div
      class="mb-1 h-[110px] rounded border border-slate-800 bg-slate-950 bg-no-repeat"
      style={previewStyle}
      aria-label="Crop slice preview"
    ></div>
    <p class="font-mono text-[10px] text-slate-500">
      src ≈ {Math.max(0, Math.round(slice.x))}, {Math.max(0, Math.round(slice.y))} ·
      {Math.max(0, Math.round(slice.w))}×{Math.max(0, Math.round(slice.h))} px
    </p>
  {:else}
    <div
      class="flex h-[110px] items-center justify-center rounded border border-slate-800 bg-slate-950 text-[10px] text-slate-500"
    >
      Set an image to preview the slice.
    </div>
  {/if}
</div>
