// The Library tab's state that outlives its screen (TvStage remounts a tab on
// every switch): the search, the sort and the folders (tvmode.js `S.libQ`,
// `S.libSort`, `S.folds`). The selected rail row is the desktop's own
// (`library/selection.svelte.ts`), so both modes agree on it for the session.
import type { DriveGame } from '../../../api';
import { librarySelection, selectRail } from '../../../library/selection.svelte';
import type { RailKey } from '../../../library/rail';
import type { LibrarySort } from '../../../library/sort';
import { DEFAULT_SORT } from './libraryHeader';
import { DEFAULT_FOLDS, type FolderId, type Folds } from './libraryRail';

export const lib = $state<{ query: string; sort: LibrarySort; folds: Folds }>({
  query: '',
  sort: DEFAULT_SORT,
  folds: { ...DEFAULT_FOLDS },
});

/** Moving through the rail selects a row at once; a new row clears the search (item 178). */
export function selectLibRail(key: RailKey): void {
  if (librarySelection.key === key) return;
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
