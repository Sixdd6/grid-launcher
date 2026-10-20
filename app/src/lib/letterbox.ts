// Whether a cover drawn with `object-fit: contain` leaves bars inside its
// frame. A card's blurred backdrop (`Image.svelte`, `backdrop`) exists only
// to fill those bars for square or wide art; behind a cover that fills the
// frame it is never seen, and a blurred copy is the most expensive thing a
// card paints (measured 2026-10-20: hiding every backdrop took a 2500-game
// platform's scroll from 35–55 to 27 ms per frame in the build container).

/** Bars thinner than this, in px, are rounding, not a letterbox. */
export const LETTERBOX_TOLERANCE_PX = 1;

/**
 * True when an image `naturalWidth` × `naturalHeight`, contained in a box
 * `boxWidth` × `boxHeight`, leaves a gap on either axis. Unknown sizes (0)
 * answer true: drawing a backdrop nobody needed costs a paint, a missing one
 * shows bare bars.
 */
export function leavesLetterbox(
  naturalWidth: number,
  naturalHeight: number,
  boxWidth: number,
  boxHeight: number,
  tolerancePx: number = LETTERBOX_TOLERANCE_PX
): boolean {
  if (!(naturalWidth > 0 && naturalHeight > 0 && boxWidth > 0 && boxHeight > 0)) return true;
  const scale = Math.min(boxWidth / naturalWidth, boxHeight / naturalHeight);
  return (
    boxWidth - naturalWidth * scale > tolerancePx || boxHeight - naturalHeight * scale > tolerancePx
  );
}
