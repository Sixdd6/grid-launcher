// TV mode's own Card size and Game labels (TV Settings › Appearance, user
// decision 2026-11-14; tvmode.js `paneRows('appearance')`, tvmode.css
// `--gcw` and the focus-label pill). Stored as `[ui]` `tv_card_size` and
// `tv_card_labels`, apart from the desktop's `card_size_*` and
// `card_labels`: changing one mode never changes the other. Pure, like
// `cards/size.ts`.

export const TV_CARD_SIZES = ['small', 'medium', 'large'] as const;
export type TvCardSize = (typeof TV_CARD_SIZES)[number];

export const TV_CARD_LABEL_MODES = ['always', 'focus'] as const;
export type TvCardLabels = (typeof TV_CARD_LABEL_MODES)[number];

/** A cover's width at the portrait shape, in rem, for every TV grid
 *  (Library, a Server platform, a Backup drive, Search results). Medium is
 *  the 10rem TV mode always used; Small shrinks less than the desktop's so
 *  covers stay readable from a couch. */
const COVER_REM: Record<TvCardSize, number> = { small: 8.5, medium: 10, large: 12.5 };

const SIZE_LABELS: Record<TvCardSize, string> = { small: 'Small', medium: 'Medium', large: 'Large' };
const LABEL_LABELS: Record<TvCardLabels, string> = { always: 'Always visible', focus: 'Visible on focus' };

/** The title line under an Always visible cover: the cover's .5rem gap and
 *  one .9rem line at line-height 1.4 (tv.css `.cv-l`). */
export const TV_LABEL_ROW_REM = 0.5 + 0.9 * 1.4;

/** The stored value, or `medium` for anything unrecognised. Matches the
 *  Rust `normalize_card_size`, including its case sensitivity. */
export function normalizeTvCardSize(raw: string): TvCardSize {
  const v = raw.trim();
  return (TV_CARD_SIZES as readonly string[]).includes(v) ? (v as TvCardSize) : 'medium';
}

/** The stored value, or `always` for anything unrecognised (a desktop
 *  `hover` included). Matches the Rust `normalize_tv_card_labels`. */
export function normalizeTvCardLabels(raw: string): TvCardLabels {
  return raw.trim() === 'focus' ? 'focus' : 'always';
}

export function tvCoverRem(size: TvCardSize): number {
  return COVER_REM[size];
}

/** The row each cover reserves for its title, in rem: none when only the
 *  focused cover's title shows, as a pill that never moves the layout. */
export function tvLabelRowRem(labels: TvCardLabels): number {
  return labels === 'focus' ? 0 : TV_LABEL_ROW_REM;
}

export function tvCardSizeLabel(size: TvCardSize): string {
  return SIZE_LABELS[size];
}

export function tvCardLabelsLabel(mode: TvCardLabels): string {
  return LABEL_LABELS[mode];
}
