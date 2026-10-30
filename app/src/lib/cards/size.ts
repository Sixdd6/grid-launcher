// Card sizing for both grids (design §5, D-UI-9). Pure: the store owns the
// persisted value, the components own the markup, this module owns the
// three numbers and the template string they turn into.
import type { CardSizeName } from '../api';
import type { CardLabels } from './labels';

export const CARD_SIZES = ['small', 'medium', 'large'] as const;
export type CardSize = CardSizeName;

/** Design §5: "sizes 120 / 160 / 200px" — the grid's minimum column width. */
const MIN_PX: Record<CardSize, number> = { small: 120, medium: 160, large: 200 };
const LABELS: Record<CardSize, string> = { small: 'Small', medium: 'Medium', large: 'Large' };

/**
 * The stored value, or `medium` for anything unrecognized. Matches the
 * Rust `normalize_card_size` exactly, including its case sensitivity, so
 * the two normalizers can never disagree about a config value.
 */
export function normalizeCardSize(raw: string): CardSize {
  const trimmed = raw.trim();
  return (CARD_SIZES as readonly string[]).includes(trimmed) ? (trimmed as CardSize) : 'medium';
}

export function cardMinPx(size: CardSize): number {
  return MIN_PX[size];
}

export function gridTemplate(size: CardSize): string {
  return `repeat(auto-fill, minmax(${MIN_PX[size]}px, 1fr))`;
}

export function cardSizeLabel(size: CardSize): string {
  return LABELS[size];
}

/**
 * How many columns the browser actually laid out. `auto-fill` means the
 * count depends on the window width, so keyboard focus movement cannot use
 * a constant the way the pre-redesign grids did: it reads the resolved
 * `grid-template-columns`, which is a space-separated list of concrete
 * track sizes once layout has run.
 *
 * Returns 1 — a single-column list — when there is no element, no view to
 * compute styles from, or no layout yet (`none`). One is the safe floor:
 * `moveFocus` treats every card as its own row, which navigates correctly
 * even if it navigates slowly.
 */
export function columnsOf(grid: HTMLElement | null): number {
  const view = grid?.ownerDocument?.defaultView;
  if (!grid || !view) return 1;
  const tracks = parseTracks(view.getComputedStyle(grid).gridTemplateColumns);
  return tracks === null ? 1 : tracks.length;
}

/**
 * The resolved track widths of a `grid-template-columns` value, in px, or
 * `null` when it is not resolved to px tracks.
 *
 * A grid inside a `hidden` view has no layout, and its computed template
 * is the specified `repeat(auto-fill, minmax(160px, 1fr))`. Split on spaces
 * that reads as three tracks, so anything that is not a plain px length
 * makes the whole value unresolved.
 */
export function parseTracks(template: string | null | undefined): number[] | null {
  if (!template || template === 'none') return null;
  const parts = template.trim().split(/\s+/).filter((t) => t.length > 0);
  if (parts.length === 0) return null;
  const tracks: number[] = [];
  for (const part of parts) {
    const match = /^(\d+(?:\.\d+)?)px$/.exec(part);
    if (!match) return null;
    const px = Number(match[1]);
    if (!(px > 0)) return null;
    tracks.push(px);
  }
  return tracks;
}

/**
 * Every card's cell is 3:4 (design §5): wide enough for a column, as tall as
 * a portrait box. The cover inside keeps its OWN shape (look-and-feel plan
 * 2026-10-30): it is fitted inside the cell, never cropped, and sits on the
 * cell's bottom edge so the titles and the next row line up whatever the
 * shape. Width over height, for the windowed grid's row arithmetic.
 */
export const CARD_CELL_ASPECT = 3 / 4;

/** The "Always visible" label under the cover: its height, and the gap
 *  above it, in px. "Visible on hover" reserves neither: its label floats
 *  over the gap below the cover. */
export const LABEL_HEIGHT_PX = 24;
export const LABEL_GAP_PX = 8;

/** The height a card adds under its cell for its label, in px. */
export function labelRowPx(labels: CardLabels): number {
  return labels === 'always' ? LABEL_GAP_PX + LABEL_HEIGHT_PX : 0;
}

/**
 * The hover bar's strip at the bottom of the cover, in px: Play/Install,
 * Cloud sync and Details sit in it, and nothing interactive sits above it.
 *
 * That keeps the band around the CARD ROOT's centre free of controls.
 * WebdriverIO clicks an element's centre, and `library-card-<id>` /
 * `game-card-<id>` are what the specs click to open Details: a control
 * there would launch or install instead. The root is the cell plus the
 * label row, so its centre is at `(cell + labelRow) / 2`, and the cover's
 * bottom is the cell's bottom whatever the cover's shape. `size.test.ts`
 * proves the strip clears the centre for every card size and both label
 * modes. Clicks on the cover above the strip, or on the empty part of a
 * cell above a short cover, fall through to the root and open Details.
 */
export const ACTION_BAR_HEIGHT_PX = 38;
