// The "Game labels" display setting (Settings › Appearance, look-and-feel
// plan 2026-10-30): whether each card's title label is always shown under
// its cover, or only on hover and keyboard focus. Pure, like `size.ts`.

export const CARD_LABEL_MODES = ['always', 'hover'] as const;
export type CardLabels = (typeof CARD_LABEL_MODES)[number];

const LABELS: Record<CardLabels, string> = {
  always: 'Always visible',
  hover: 'Visible on hover',
};

/** The stored value, or `always` for anything unrecognised. Matches the
 *  Rust `normalize_card_labels`, including its case sensitivity. */
export function normalizeCardLabels(raw: string): CardLabels {
  return raw.trim() === 'hover' ? 'hover' : 'always';
}

export function cardLabelsLabel(mode: CardLabels): string {
  return LABELS[mode];
}
