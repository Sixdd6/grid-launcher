// Q4: an installed row with no server rom id (imported from the Python app,
// or installed before rom ids were recorded). Details offers two actions for
// it: link it to a server game, or remove it from the library and keep its
// files. This module holds the rules; the components only render them.
import type { GameSummary, InstalledGame } from '../api';
import { filterServerGames } from '../server/search';

/** The unlinked actions show for a registry row that has no rom id. A server
 *  subject with no row has nothing to link or remove. */
export function showsUnlinkedActions(romId: number | null, row: InstalledGame | null): boolean {
  return romId === null && row !== null && row.rom_id === null;
}

/** The picker's list: `games` filtered by the Server search rule (title,
 *  platform or genre), minus any rom another installed row already holds —
 *  the backend refuses those, so offering them would only produce an error. */
export function linkCandidates(
  games: GameSummary[],
  query: string,
  heldRomIds: ReadonlySet<number>,
): GameSummary[] {
  return filterServerGames(
    games.filter((g) => !heldRomIds.has(g.id)),
    query,
  );
}

/** Everything up to the last `/` or `\`; `''` for a bare name. */
function parentDir(path: string): string {
  const cut = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return cut > 0 ? path.slice(0, cut) : '';
}

/** The folder the confirm names as staying on disk: the row's own game
 *  folder when it stores one, else the folder of its single file. `''` when
 *  the row stores no path at all. */
export function keptFolder(row: InstalledGame): string {
  const folder = [row.extracted_dir, row.multi_file_game_dir, row.native_game_dir].find(
    (p) => (p ?? '').trim() !== '',
  );
  if (folder) return folder;
  const file = [row.extracted_path, row.archive_path, row.native_executable_path, row.ps3_iso_path].find(
    (p) => (p ?? '').trim() !== '',
  );
  return file ? parentDir(file) : '';
}

/** The confirm's sentence. It always says the files stay. */
export function removeConfirmMessage(title: string, folder: string): string {
  const head = `Remove “${title}” from your library?`;
  return folder === '' ? `${head} No files are deleted.` : `${head} Its files stay on disk in ${folder}.`;
}
