// "Cover all monitors" math (§4c.5). Given the layout's mm extent and the
// image's natural aspect, return the smallest aspect-preserving image
// rectangle (in core-canvas px) that fully covers the canvas — centred.
// Returns `null` when inputs are degenerate.

import type { CanvasLayout } from './types';

export type CoverTransform = {
  offset: [number, number];
  imageSizePx: [number, number];
};

export function snapToCoverTransform(
  layout: CanvasLayout,
  imageNaturalW: number,
  imageNaturalH: number,
): CoverTransform | null {
  if (imageNaturalW <= 0 || imageNaturalH <= 0) return null;
  const coreCanvasW = layout.totalMmW * layout.coreMmToPx;
  const coreCanvasH = layout.totalMmH * layout.coreMmToPx;
  if (coreCanvasW <= 0 || coreCanvasH <= 0) return null;
  const aspect = imageNaturalW / imageNaturalH;
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
  return {
    offset: [Math.round(offX), Math.round(offY)],
    imageSizePx: [Math.max(1, Math.round(w)), Math.max(1, Math.round(h))],
  };
}
