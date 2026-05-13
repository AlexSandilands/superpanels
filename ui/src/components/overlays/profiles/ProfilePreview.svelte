<script lang="ts">
  // Profile thumbnail preview that draws the source image cover-fitted over
  // the monitor union (mm-space) and overlays each monitor rectangle at its
  // authored physical position. All elements share a single mm→px scale so
  // the monitors visually sit on the image where they will physically crop
  // it on apply.

  import type { TopologyRect } from '$lib/profile-topology';

  type Props = {
    rects: TopologyRect[];
    imageUrl: string | null;
    naturalDims: { w: number; h: number } | null;
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

    let imgX = minX;
    let imgY = minY;
    let imgW = bbW;
    let imgH = bbH;
    if (naturalDims && naturalDims.w > 0 && naturalDims.h > 0) {
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

    // Union covers both the image rect (which already covers the monitor
    // bbox) and the monitors themselves; in cover-fit cases the image is
    // the outer bound, but we keep this general for safety.
    const ux0 = Math.min(minX, imgX);
    const uy0 = Math.min(minY, imgY);
    const ux1 = Math.max(maxX, imgX + imgW);
    const uy1 = Math.max(maxY, imgY + imgH);
    const uW = Math.max(1, ux1 - ux0);
    const uH = Math.max(1, uy1 - uy0);

    const innerW = Math.max(1, width - padding * 2);
    const innerH = Math.max(1, height - padding * 2);
    const s = Math.min(innerW / uW, innerH / uH);

    const offX = padding + (innerW - uW * s) / 2 - ux0 * s;
    const offY = padding + (innerH - uH * s) / 2 - uy0 * s;

    return {
      img: { x: imgX * s + offX, y: imgY * s + offY, w: imgW * s, h: imgH * s },
      monitors: rects.map((r) => ({
        x: r.x * s + offX,
        y: r.y * s + offY,
        w: r.w * s,
        h: r.h * s,
      })),
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
    {#if imageUrl}
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
