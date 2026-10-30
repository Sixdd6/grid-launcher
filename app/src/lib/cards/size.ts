// Card sizing for both grids (design §5, D-UI-9). Pure: the store owns the
// persisted value, the components own the markup, this module owns the
// three numbers and the template string they turn into.
import type { CardSizeName } from '../api';
import type { CardLabels } from './labels';

export const CARD_SIZES = ['small', 'medium', 'large'] as const;
export type CardSize = CardSizeName;

/** Design §5: "sizes 120 / 160 / 200px" — Equal width's minimum column width,
 *  and the width of a portrait cover in Equal height. */
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

export function cardSizeLabel(size: CardSize): string {
  return LABELS[size];
}

/**
 * Equal height's target row height for a size: the 3:4 height of the size's
 * minimum width (Small 160, Medium 213, Large 267), so a portrait cover is
 * the size it always was (`cards/layout.ts`).
 */
export function targetRowPx(size: CardSize): number {
  return Math.round(MIN_PX[size] / CARD_CELL_ASPECT);
}

/**
 * The portrait shape (width over height), design §5's 3:4: what a cover is
 * laid out as before its own shape is known, and what each card size's
 * target height is derived from.
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
 * there would launch or install instead. The root is the cover plus the
 * label row, so its centre is at `(cover + labelRow) / 2`; every cover is at
 * least `MIN_COVER_PX` tall (`cards/layout.ts`). `size.test.ts` proves the
 * strip clears the centre at that height in both label modes. Clicks on the
 * cover above the strip fall through to the root and open Details.
 */
export const ACTION_BAR_HEIGHT_PX = 38;
