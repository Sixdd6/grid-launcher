// Scroll targets (checklist items 107–112; tvmode.js `reveal`). Pure: every
// move scrolls by one of these rules, never by the browser's "just enough".
//
// - A row that scrolls sideways keeps the focused item one row padding inside
//   its edges (ruling 3: 1.2rem, the padding of a row inside a panel), so a
//   grown, ringed cover is never cut at the left or right.
// - On Discover, moving into a shelf brings its title to the top of the rows
//   area; the first shelf sits at the very top.
// - Anywhere else the focused item stays 1.2rem inside the scrolling area
//   (room for its ring and lift); an item taller than the area lines up with
//   its top; the area's first item scrolls it to the very top and its last
//   item to the very bottom.

/** Within this many pixels a scroll is not worth starting. */
const SLACK = 1;

export type HorizontalInput = {
  trackLeft: number;
  trackRight: number;
  itemLeft: number;
  itemRight: number;
  /** The row's own padding-left: the margin the item keeps. */
  edge: number;
  scrollLeft: number;
  scrollWidth: number;
  clientWidth: number;
};

/** The sideways row's new scrollLeft, or null to leave it. */
export function revealX(o: HorizontalInput): number | null {
  let x = o.scrollLeft;
  if (o.itemLeft - o.trackLeft < o.edge) x += o.itemLeft - o.trackLeft - o.edge;
  else if (o.trackRight - o.itemRight < o.edge) x += o.itemRight - o.trackRight + o.edge;
  x = Math.max(0, Math.min(x, o.scrollWidth - o.clientWidth));
  return Math.abs(x - o.scrollLeft) > SLACK ? x : null;
}

export type VerticalInput = {
  areaTop: number;
  areaBottom: number;
  itemTop: number;
  itemBottom: number;
  /** The root font size in pixels. */
  rem: number;
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  /** The item is the area's first / last focusable. */
  first: boolean;
  last: boolean;
  /**
   * Set on Discover's rows area: the top of the shelf holding the item, or
   * null when it is the first shelf.
   */
  shelfTop?: number | null;
};

/** Inside a scrolling area, the focused item keeps this many rem of room. */
export const INSET_REM = 1.2;

/** The area's new scrollTop, or null to leave it. */
export function revealY(o: VerticalInput): number | null {
  let y = o.scrollTop;
  if (o.shelfTop !== undefined) {
    y = o.shelfTop === null ? 0 : y + (o.shelfTop - o.areaTop);
  } else {
    const pad = INSET_REM * o.rem;
    const top = o.itemTop - pad;
    const bot = o.itemBottom + pad;
    if (top < o.areaTop) y += top - o.areaTop;
    else if (bot > o.areaBottom) y += Math.min(bot - o.areaBottom, top - o.areaTop);
    if (o.first) y = 0;
    else if (o.last) y = o.scrollHeight;
  }
  y = Math.max(0, Math.min(y, o.scrollHeight - o.clientHeight));
  return Math.abs(y - o.scrollTop) > SLACK ? y : null;
}

/**
 * Details' summary reads like a page (Down / Up scroll it while more of it is
 * out of view that way, then move on): how far to scroll, or null to move.
 */
export function readStep(dir: 'up' | 'down', itemTop: number, itemBottom: number, areaTop: number, areaBottom: number, rem: number): number | null {
  const step = 6 * rem;
  if (dir === 'down' && itemBottom > areaBottom + 1) return Math.min(step, itemBottom - areaBottom + INSET_REM * rem);
  if (dir === 'up' && itemTop < areaTop - 1) return -Math.min(step, areaTop - itemTop + INSET_REM * rem);
  return null;
}
