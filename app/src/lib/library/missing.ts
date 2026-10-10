// 9c moved-file detection: an installed row whose files are not on disk.
// The backend's reconcile pass (grid-core `library/reconcile.rs`) sets
// `missing` on every `list_installed` row; it is never stored. A moved game
// the pass could find was already re-pointed and is not missing. This module
// holds the rules the Library card and Details render.

/** The card badge's text. */
export const MISSING_BADGE_TEXT = 'MISSING';

/** Why Play and Uninstall are blocked for a missing game. */
export const MISSING_REASON =
  'The game files were not found. Locate them, or remove the game from your library.';

/** True for a row the backend flagged. Rows from other commands carry no flag. */
export function isMissing(row: { missing?: boolean } | null | undefined): boolean {
  return row?.missing === true;
}

/** The card's primary button: a missing game opens Details to locate it. */
export function cardPrimaryLabel(installed: boolean, missing: boolean): string {
  if (!installed) return 'Install';
  return missing ? 'Locate…' : 'Play';
}

export type MissingActions = {
  /** Details shows "Files not found" with Locate… and Remove from library. */
  showMissing: boolean;
  /** `''` when Play is allowed, else the reason it is disabled. */
  playBlocked: string;
  /** `''` when Uninstall is allowed, else the reason it is disabled. */
  uninstallBlocked: string;
};

export function missingActions(row: { missing?: boolean } | null | undefined): MissingActions {
  const missing = isMissing(row);
  return {
    showMissing: missing,
    playBlocked: missing ? MISSING_REASON : '',
    uninstallBlocked: missing ? MISSING_REASON : '',
  };
}
