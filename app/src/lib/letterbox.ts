// How a cover sits in its 3:4 frame. A card's blurred backdrop (`Image.svelte`,
// `backdrop`) exists only to fill bars for square or wide art; behind a cover
// that fills the frame it is never seen, and a blurred copy is the most
// expensive thing a card paints (measured 2026-10-20: hiding every backdrop
// took a 2500-game platform's scroll from 35–55 to 27 ms per frame in the
// build container).
//
// Box art is rarely exactly 3:4. ScreenScraper's 198×272 (0.728) leaves a
// ~2px bar each side of a Medium card, and before `crop` existed that sliver
// cost every such card a full backdrop: on a real 1,973-game Arcade platform,
// all 35 cards on screen drew one, and cold scrolling ran at 34–37 ms a frame
// against 28–30 without (2026-10-28).

/** Bars thinner than this, in px, are rounding, not a letterbox. */
export const LETTERBOX_TOLERANCE_PX = 1;

/** Art whose bars would take at most this share of the frame on their axis
 *  fills the frame instead (`object-fit: cover`), losing at most half of it
 *  from each edge. 6% takes in the common near-3:4 box art (0.711–0.79). */
export const CROP_TO_FILL_MAX = 0.06;

/**
 * - `fits` — no bars worth drawing.
 * - `crop` — near 3:4: fill the frame, trimming a sliver, and draw no backdrop.
 * - `letterbox` — real bars (square, wide or very tall art): contain it and
 *   fill the bars with the backdrop.
 */
export type CoverFit = 'fits' | 'crop' | 'letterbox';

/**
 * How an image `naturalWidth` × `naturalHeight` sits in a box `boxWidth` ×
 * `boxHeight` when contained. Unknown sizes (0) answer `letterbox`: drawing a
 * backdrop nobody needed costs a paint, a missing one shows bare bars.
 */
export function coverFit(
  naturalWidth: number,
  naturalHeight: number,
  boxWidth: number,
  boxHeight: number,
  tolerancePx: number = LETTERBOX_TOLERANCE_PX,
  cropMax: number = CROP_TO_FILL_MAX
): CoverFit {
  if (!(naturalWidth > 0 && naturalHeight > 0 && boxWidth > 0 && boxHeight > 0)) return 'letterbox';
  const scale = Math.min(boxWidth / naturalWidth, boxHeight / naturalHeight);
  const gapX = boxWidth - naturalWidth * scale;
  const gapY = boxHeight - naturalHeight * scale;
  if (gapX <= tolerancePx && gapY <= tolerancePx) return 'fits';
  return gapX / boxWidth <= cropMax && gapY / boxHeight <= cropMax ? 'crop' : 'letterbox';
}
