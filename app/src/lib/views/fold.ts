// The folding grid header (Server and Library share it). Pure: the
// hysteresis, the measured heights and the search placeholders.
// `FoldingHeader.svelte` wires these to the scroller.

import type { RailKey } from '../library/rail';

/** Past this many px of scroll the header folds into its toolbar row. */
export const FOLD_AT = 32;
/** Back within this many px of the top it unfolds. The gap between the two
 *  stops the header flickering when the scroll rests near one threshold. */
export const UNFOLD_AT = 8;

/** The header's vertical padding, unfolded and folded (each side). */
export const PAD_UNFOLDED_PX = 14;
export const PAD_FOLDED_PX = 8;
/** The space between the title row and the toolbar row, unfolded. */
export const ROW_GAP_PX = 12;
/** The panel's border, top plus bottom. */
export const BORDER_PX = 2;
/** Room left between the folded header and a card brought into view. */
export const CLEARANCE_PX = 8;

/** The header's next folded state for the scroller's `scrollTop`. */
export function nextFolded(folded: boolean, scrollTop: number): boolean {
  if (!folded && scrollTop > FOLD_AT) return true;
  if (folded && scrollTop < UNFOLD_AT) return false;
  return folded;
}

/**
 * The header's unfolded height, which the scroller reserves above the grid.
 * Built from the title row's and toolbar's own heights rather than read off
 * the header, whose height animates while it folds and unfolds — reading
 * that mid-fold would move every card.
 */
export function unfoldedHeight(titleRowPx: number, toolbarPx: number): number {
  return titleRowPx + ROW_GAP_PX + toolbarPx + 2 * PAD_UNFOLDED_PX + BORDER_PX;
}

/** The folded header's height plus clearance: how far below the scroller's
 *  top a card brought into view by the keyboard must sit. */
export function foldedClearance(toolbarPx: number): number {
  return toolbarPx + 2 * PAD_FOLDED_PX + BORDER_PX + CLEARANCE_PX;
}

/** The Server search field's placeholder; folded, it names the platform,
 *  since the title row that did is hidden. */
export function serverSearchPlaceholder(folded: boolean, platformLabel: string): string {
  if (!folded || platformLabel.trim() === '') return 'Search title, platform or genre';
  return `Search ${platformLabel.trim()}…`;
}

/** The Library search field's placeholder; folded, it names the rail entry
 *  in view, since the title row that did is hidden. */
export function librarySearchPlaceholder(folded: boolean, key: RailKey, label: string): string {
  if (key.startsWith('drive:')) return folded ? `Search ${label.trim()}…` : 'Search games on this drive';
  if (!folded) return 'Search installed games';
  if (key === 'all') return 'Search your library…';
  if (key === 'recent') return 'Search recent games…';
  if (key === 'updates') return 'Search games with updates…';
  return `Search ${label.trim()}…`;
}

/** "1 game" / "N games" — the Library title row's count. */
export function gameCountLine(count: number): string {
  return count === 1 ? '1 game' : `${count} games`;
}
