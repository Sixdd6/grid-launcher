// The app's icon artwork. One pure module so the paths are unit-testable and
// so `Icon.svelte` stays a five-line shell.
//
// Every path is hand-authored on ONE 24×24 grid, optically centred on
// (12, 12), drawn to read at a 1.5-unit stroke. The old PySide6 app's
// `assets/svg/` files are deliberately NOT reused: their viewBoxes run from
// `-0.5 0 7 7` to `0 0 1000 1000`, so re-fitting them would still leave each
// icon inconsistent. Nothing here references those files.
//
// The marks do not share one optical weight — like Feather's icon set,
// per-mark sizing follows the shape, not a fixed span. See each icon's own
// docstring below for its exact bounds.
//
// No colour appears in this module. The component paints with
// `currentColor`, so an icon is always the colour of the text around it.

export const ICONS = {
  /** Two diagonals spanning 12 of the 24 units. Close, and (at 14) dismiss and remove. */
  close: 'M6 6l12 12M18 6L6 18',

  /** Apex at x=8.5, arms to x=15.5, so the mark is centred on x=12. */
  chevronLeft: 'M15.5 5L8.5 12l7 7',

  /** The mirror of `chevronLeft`, same span and same centre. */
  chevronRight: 'M8.5 5l7 7-7 7',

  /** A 15-unit shaft on the centre line with a 6.5-unit head. */
  arrowLeft: 'M19.5 12H4.5M11 5.5L4.5 12l6.5 6.5',

  /**
   * Outline cloud: a flat base at y=17, a 4-radius right lobe, a shallow
   * 5.2-radius top and a 3.3-radius left lobe. Spans x 3.8–20.2, y 6.9–17.
   */
  cloud: 'M6.2 17h10a4 4 0 0 0 0-8h-.6A5.2 5.2 0 0 0 6.4 10.6 3.3 3.3 0 0 0 6.2 17z',

  /**
   * Solid five-point star. Outer radius 8 and inner radius 3.2 about
   * (12, 12.4), first point at -90° (straight up), then every 36°.
   */
  star: 'M12 4.4L13.88 9.81L19.61 9.93L15.04 13.39L16.7 18.87L12 15.6L7.3 18.87L8.96 13.39L4.39 9.93L10.12 9.81Z',

  /**
   * A shaft and head down the centre line into an open tray: 2-unit side
   * walls (taller than the 1.5-unit corner radius) meet a flat base at
   * y=20.5.
   */
  download: 'M12 3.5V14M7.5 9.5L12 14L16.5 9.5M4.5 17v2a1.5 1.5 0 0 0 1.5 1.5h12a1.5 1.5 0 0 0 1.5-1.5v-2',

  /** Solid right-pointing triangle; its centroid sits on x=11.3, y=12. */
  play: 'M7.5 4.5L19 12L7.5 19.5Z',

  /** The brandmark: four 6×6 cells with a 4-unit gutter, spanning 4–20. */
  grid: 'M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z',
} as const;

export type IconName = keyof typeof ICONS;

/**
 * The solid marks. Their path takes `fill="currentColor" stroke="none"`;
 * every other icon takes the root's 1.5-unit `stroke="currentColor"`. A
 * filled path must not also take the stroke, or the shape thickens by half a
 * unit on every edge and stops matching the outline icons beside it.
 */
export const FILLED_ICONS: readonly IconName[] = ['star', 'play'];
