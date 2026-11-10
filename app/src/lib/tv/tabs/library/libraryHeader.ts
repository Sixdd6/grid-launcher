// The TV Library grid's header, sort and empty states (checklist items 185,
// 187–191, 421–423; tvmode.js `libPaint`, `libList`, `sortSheet`). Pure.
import type { DriveGame, DriveInfo, InstalledGame } from '../../../api';
import { sizeText } from '../../../backups/dialog';
import { DRIVE_SORTS, driveLabel, driveSortLabel, type DriveSort } from '../../../library/drive';
import type { RailKey } from '../../../library/rail';
import { sortGames, titleContains, type LibrarySort } from '../../../library/sort';

/** The "Sort by" list, in order, with Recently played the default (item 188). */
export const TV_SORTS: readonly LibrarySort[] = ['played', 'installed', 'title', 'platform'];
export const DEFAULT_SORT: LibrarySort = 'played';
export const SORT_TITLE = 'Sort by';

/** Y's keyboard title (item 187). */
export const SEARCH_TITLE = 'Search your library';

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** "<n> games", "1 game" (item 185). */
export function countLine(n: number): string {
  return plural(n, 'game');
}

/** Whether a query narrows the grid: a blank one is no filter (consultant ruling). */
export function searching(query: string): boolean {
  return query.trim() !== '';
}

/** The installed rows a filter shows, searched and sorted (tvmode.js `libList`). */
export function libraryGrid(rows: InstalledGame[], query: string, sort: LibrarySort): InstalledGame[] {
  return sortGames(
    rows.filter((r) => titleContains(r.title, query)),
    sort,
  );
}

/**
 * LT on a Backup drive (ruling E5): the desktop drive grid's sorts (Title,
 * Platform, Recently backed up, Largest first), remembered apart from the
 * Library sort for the session, Title first.
 */
export const DEFAULT_DRIVE_SORT: DriveSort = 'title';

export function driveSortItems(current: DriveSort): { sort: DriveSort; label: string; checked: boolean }[] {
  return DRIVE_SORTS.map((sort) => ({ sort, label: driveSortLabel(sort), checked: sort === current }));
}

export type EmptyState = {
  title: string;
  /** The line under it; "(Y)" in it stands for the Y button's glyph. */
  body: string;
};

/** No match for a search (item 189). */
export function noMatch(query: string): EmptyState {
  return { title: `No games match “${query.trim()}”`, body: 'Press (Y) to change the search.' };
}

/** An empty filter (items 189–191; tvmode.js `libPaint`). */
export function emptyState(key: RailKey, query: string): EmptyState {
  if (searching(query)) return noMatch(query);
  if (key === 'updates') return { title: 'Everything is up to date', body: 'Games with a newer copy on your server show here.' };
  return { title: 'No games here yet', body: 'Install games from the Server tab.' };
}

/**
 * An empty drive grid: the drive went away while selected (item 423), a
 * search with no match, or a drive with nothing on it yet.
 */
export function driveEmptyState(name: string, connected: boolean, query: string): EmptyState {
  if (!connected) return { title: `${name} is not connected`, body: 'Plug it in to see its games.' };
  // The Game backups card's words (library/drive.ts `driveEmptyText`).
  if (searching(query)) return { title: `No games on this drive match “${query.trim()}”`, body: 'Press (Y) to change the search.' };
  return { title: 'Nothing on this drive yet', body: 'Backup games from Server or Library.' };
}

/**
 * A drive's header line (item 421): "<n> games · <size> · on <label>",
 * counting the games the grid shows and their size.
 */
export function driveCountLine(drive: Pick<DriveInfo, 'label' | 'mount'>, shown: readonly Pick<DriveGame, 'size'>[]): string {
  const bytes = shown.reduce((sum, g) => sum + g.size, 0);
  return `${countLine(shown.length)} · ${sizeText(bytes)} · on ${driveLabel(drive)}`;
}

/** A drive header's chips (item 421): the emulators by name, and how many systems have firmware. */
export function driveChips(drive: Pick<DriveInfo, 'pack'>): { emulators: string; firmware: string } {
  const names: string[] = [];
  for (const e of drive.pack?.emulators ?? []) {
    const n = e.name.trim();
    if (n !== '' && !names.includes(n)) names.push(n);
  }
  const systems = new Set((drive.pack?.firmware ?? []).map((f) => f.trim()).filter((f) => f !== '')).size;
  return { emulators: names.length ? names.join(', ') : 'none', firmware: plural(systems, 'system') };
}

/** Remove <n> from drive… in select mode on a drive (item 451). */
export function removedFromDriveConfirm(n: number, pack: string): { title: string; body: string; ok: string } {
  return {
    title: `Remove ${countLine(n)} from ${pack}?`,
    body: 'Their files leave the drive. Games installed on this computer stay.',
    ok: 'Remove from drive',
  };
}
