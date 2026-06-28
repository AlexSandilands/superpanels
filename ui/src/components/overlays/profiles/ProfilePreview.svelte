<script lang="ts">
  // Profile thumbnail preview. For a slideshow it cover-fits the single source
  // image over the monitor union (mm-space). For a standard profile it stacks
  // each layer at its own mm-space rect (bottom→top), matching the live canvas.
  // Monitor rectangles overlay at their authored physical positions; everything
  // shares one mm→px scale so the monitors sit where they will crop on apply.

  import type { TopologyRect } from '$lib/profile-topology';

  /** A standard profile's layer: an image drawn to fill its mm-space rect. */
  export type PreviewLayer = {
    url: string | null;
    rect: { x_mm: number; y_mm: number; w_mm: number; h_mm: number };
  };

  type Props = {
    rects: TopologyRect[];
    imageUrl: string | null;
    naturalDims: { w: number; h: number } | null;
    /** When set, layers are stacked instead of the single cover-fit image. */
    layers?: PreviewLayer[] | null;
    width: number;
    height: number;
    padding?: number;
    background: string;
    disabled?: boolean;
  };

  let {
    rects,
    imageUrl,
    naturalDims,
    layers = null,
    width,
    height,
    padding = 14,
    background,
    disabled = false,
  }: Props = $props();

  const layout = $derived.by(() => {
    if (rects.length === 0) return null;

    const xs = rects.flatMap((r) => [r.x, r.x + r.w]);
    const ys = rects.flatMap((r) => [r.y, r.y + r.h]);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const bbW = Math.max(1, maxX - minX);
    const bbH = Math.max(1, maxY - minY);

    const useLayers = layers !== null && layers.length > 0;

    // Single cover-fit image rect (slideshow path).
    let imgX = minX;
    let imgY = minY;
    let imgW = bbW;
    let imgH = bbH;
    if (!useLayers && naturalDims && naturalDims.w > 0 && naturalDims.h > 0) {
      const aspect = naturalDims.w / naturalDims.h;
      let w = bbW;
      let h = w / aspect;
      if (h < bbH) {
        h = bbH;
        w = h * aspect;
      }
      imgW = w;
      imgH = h;
      imgX = minX + (bbW - w) / 2;
      imgY = minY + (bbH - h) / 2;
    }

    // Union covers the monitors plus whatever images sit over them — the single
    // cover-fit rect, or every layer rect.
    const imgXs = useLayers
      ? (layers ?? []).flatMap((l) => [l.rect.x_mm, l.rect.x_mm + l.rect.w_mm])
      : [imgX, imgX + imgW];
    const imgYs = useLayers
      ? (layers ?? []).flatMap((l) => [l.rect.y_mm, l.rect.y_mm + l.rect.h_mm])
      : [imgY, imgY + imgH];
    const ux0 = Math.min(minX, ...imgXs);
    const uy0 = Math.min(minY, ...imgYs);
    const ux1 = Math.max(maxX, ...imgXs);
    const uy1 = Math.max(maxY, ...imgYs);
    const uW = Math.max(1, ux1 - ux0);
    const uH = Math.max(1, uy1 - uy0);

    const innerW = Math.max(1, width - padding * 2);
    const innerH = Math.max(1, height - padding * 2);
    const s = Math.min(innerW / uW, innerH / uH);

    const offX = padding + (innerW - uW * s) / 2 - ux0 * s;
    const offY = padding + (innerH - uH * s) / 2 - uy0 * s;
    const project = (x: number, y: number, w: number, h: number) => ({
      x: x * s + offX,
      y: y * s + offY,
      w: w * s,
      h: h * s,
    });

    return {
      img: project(imgX, imgY, imgW, imgH),
      layers: useLayers
        ? (layers ?? []).map((l) => ({
            url: l.url,
            ...project(l.rect.x_mm, l.rect.y_mm, l.rect.w_mm, l.rect.h_mm),
          }))
        : null,
      monitors: rects.map((r) => project(r.x, r.y, r.w, r.h)),
    };
  });
</script>

<div
  class="preview"
  style:width="{width}px"
  style:height="{height}px"
  style:background
  style:opacity={disabled ? 0.5 : 1}
  style:filter={disabled ? 'grayscale(0.7)' : 'none'}
>
  {#if layout}
    {#if layout.layers}
      {#each layout.layers as l, i (i)}
        {#if l.url}
          <img
            class="img"
            src={l.url}
            alt=""
            style:left="{l.x}px"
            style:top="{l.y}px"
            style:width="{l.w}px"
            style:height="{l.h}px"
          />
        {/if}
      {/each}
    {:else if imageUrl}
      <img
        class="img"
        src={imageUrl}
        alt=""
        style:left="{layout.img.x}px"
        style:top="{layout.img.y}px"
        style:width="{layout.img.w}px"
        style:height="{layout.img.h}px"
      />
    {/if}
    {#each layout.monitors as m, i (i)}
      <div
        class="monitor"
        style:left="{m.x}px"
        style:top="{m.y}px"
        style:width="{m.w}px"
        style:height="{m.h}px"
      ></div>
    {/each}
  {/if}
</div>

<style>
  .preview {
    position: relative;
    overflow: hidden;
  }
  .img {
    position: absolute;
    object-fit: fill;
    pointer-events: none;
    user-select: none;
  }
  .monitor {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid oklch(1 0 0 / 0.9);
    border-radius: 3px;
    background: oklch(1 0 0 / 0.05);
    box-shadow:
      0 0 0 1px oklch(0 0 0 / 0.35),
      inset 0 0 0 1px oklch(0 0 0 / 0.25);
    pointer-events: none;
  }
</style>
