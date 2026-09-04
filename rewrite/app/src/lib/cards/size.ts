// Card sizing for both grids (design §5, D-UI-9). Pure: the store owns the
// persisted value, the components own the markup, this module owns the
// three numbers and the template string they turn into.
import type { CardSizeName } from '../api';

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
  const template = view.getComputedStyle(grid).gridTemplateColumns;
  if (!template || template === 'none') return 1;
  const tracks = template.trim().split(/\s+/).filter((t) => t.length > 0);
  return Math.max(1, tracks.length);
}
