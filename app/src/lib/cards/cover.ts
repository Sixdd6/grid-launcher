// Which of a game's two covers a grid card shows. Pure: `GameCard.svelte`
// passes in what it knows (its laid-out width, the display's pixel ratio,
// whether the large cover is already on disk) and renders the answer.
//
// RomM serves every cover twice: `path_cover_small`, about 200px wide (some
// are as small as 74×96), and `path_cover_large`, about 500px wide and up to
// 810×1080. A card is 90–520 CSS px wide, so on a HiDPI display it is drawn
// 180–1000 device px wide: the small cover, stretched to that, was blurry at
// every size but the smallest (2026-11-08).

/** Both cover URLs of one game; `null` where the server has none. */
export type CoverPair = { small: string | null; large: string | null };

/**
 * The widest a card may be drawn, in DEVICE pixels, and still show the small
 * cover: about the small cover's own width, so it is never stretched by more
 * than a few percent. Anything wider shows the large cover.
 */
export const SMALL_COVER_MAX_PX = 200;

/** The pair, with an empty URL (the registry stores `''` for none) as `null`. */
export function coverPair(small: string | null | undefined, large: string | null | undefined): CoverPair {
  return { small: small || null, large: large || null };
}

/**
 * The cover a card `cssWidth` px wide shows at `pixelRatio`.
 *
 * - The small cover when the card is drawn at most `SMALL_COVER_MAX_PX`
 *   device px wide (portrait art in Small or Medium cards on a 1× display):
 *   it is a fraction of the bytes and looks the same there.
 * - The large cover otherwise, and also whenever it is already on disk
 *   (`largeKnown`): a card that shrinks back under the limit (a window made
 *   narrower, a row re-justified) keeps what it shows instead of fetching
 *   the small one on top.
 * - Whichever exists when only one does; `null` when neither does.
 */
export function pickCover(pair: CoverPair, cssWidth: number, pixelRatio: number, largeKnown = false): string | null {
  if (!pair.large) return pair.small;
  if (!pair.small || largeKnown) return pair.large;
  const ratio = Number.isFinite(pixelRatio) && pixelRatio > 0 ? pixelRatio : 1;
  return cssWidth * ratio <= SMALL_COVER_MAX_PX ? pair.small : pair.large;
}

/**
 * What to show after `failed` did not load: the pair's other cover, when
 * that exists and is a different URL; otherwise `null` (the placeholder).
 */
export function coverFallback(pair: CoverPair, failed: string | null): string | null {
  if (!failed) return null;
  const other = failed === pair.large ? pair.small : failed === pair.small ? pair.large : null;
  return other && other !== failed ? other : null;
}
