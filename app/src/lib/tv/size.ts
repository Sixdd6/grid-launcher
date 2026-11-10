// TV mode's one size rule (ruling 1, checklist item 2): every measure is in
// rem, and the root font is 1/54 of the screen height, capped at 1/80 of its
// width so squarer screens still fit the layout (20px at 1080p). tv.css
// applies the same rule; this is its testable twin.

export const HEIGHT_DIVISOR = 54;
export const WIDTH_DIVISOR = 80;

export function rootFontPx(viewportWidth: number, viewportHeight: number): number {
  return Math.min(viewportHeight / HEIGHT_DIVISOR, viewportWidth / WIDTH_DIVISOR);
}

/** The three bands' heights in rem (item 5). */
export const TOP_BAR_REM = 4.6;
export const HINT_BAR_REM = 3.6;

/** No text smaller than this (ruling 2, item 3). */
export const MIN_TEXT_REM = 0.85;
