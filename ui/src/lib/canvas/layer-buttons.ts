// Geometry for the on-layer button cluster (top-right corner, laid out right→
// left: remove, snap-width, snap-height). hit-test.ts measures hit centres in
// from the layer's top-right corner; CanvasImageLayers paints the buttons from
// the same centres. Both import these so they can't silently desync.

/** Rendered button width/height, px. */
export const LAYER_BTN_SIZE = 20;
// The corner (remove) button keeps an equal 10px inset from the top and right
// edges; the snap buttons step left at a fixed 26px (20px button + 6px gap).
/** Button-centre Y, measured down from the layer's top edge, px. */
export const LAYER_BTN_TOP = 20;
/** Button-centre X, measured in from the layer's right edge, px. */
export const REMOVE_CX = 20;
export const SNAP_W_CX = 46;
export const SNAP_H_CX = 72;
