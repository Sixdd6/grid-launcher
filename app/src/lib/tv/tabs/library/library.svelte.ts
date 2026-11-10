// The Library tab's state that outlives its screen (TvStage remounts a tab on
// every switch): the search, the sort and the folders (tvmode.js `S.libQ`,
// `S.libSort`, `S.folds`). The selected rail row is the desktop's own
// (`library/selection.svelte.ts`), so both modes agree on it for the session.
import type { DriveGame } from '../../../api';
import { librarySelection, selectRail } from '../../../library/selection.svelte';
import { driveRailKey, type RailKey } from '../../../library/rail';
import type { LibrarySort } from '../../../library/sort';
import { selEnd } from '../../select.svelte';
import { DEFAULT_DRIVE_SORT, DEFAULT_SORT } from './libraryHeader';
import type { DriveSort } from '../../../library/drive';
import { DEFAULT_FOLDS, railRowKey, type FolderId, type Folds } from './libraryRail';
import { rememberFocus, showTab } from '../../nav.svelte';

export const lib = $state<{ query: string; sort: LibrarySort; driveSort: DriveSort; folds: Folds }>({
  query: '',
  sort: DEFAULT_SORT,
  // A Backup drive's sort, apart from the Library's (ruling E5).
  driveSort: DEFAULT_DRIVE_SORT,
  folds: { ...DEFAULT_FOLDS },
});

/** Moving through the rail selects a row at once; a new row clears the search (item 178). */
export function selectLibRail(key: RailKey): void {
  if (librarySelection.key === key) return;
  // Another rail row leaves select mode, as the desktop's does.
  selEnd();
  selectRail(key);
  lib.query = '';
}

/** A on a folder header opens or closes it (item 180). */
export function toggleFold(id: FolderId): void {
  lib.folds[id] = !lib.folds[id];
}

/** Each drive's games as last read, so the grid draws at once when the tab
 *  comes back (from Details, another tab). Not reactive: a cache. */
export const driveGamesCache = new Map<string, DriveGame[]>();

/** The Library on a Backup drive: its folder open, its row selected and
 *  focused when the tab next shows (items 415, 505; tvmode.js `showDrive`). */
export function selectDriveRow(key: string): void {
  const rail = driveRailKey(key);
  selectLibRail(rail);
  lib.folds.drives = true;
  rememberFocus('tab:library', railRowKey(rail));
}

/** Settings › a drive › Show in Library (item 505): pages close, the
 *  Library shows on that drive. */
export function showDriveInLibrary(key: string): void {
  selectDriveRow(key);
  void showTab('library', { again: true });
}
