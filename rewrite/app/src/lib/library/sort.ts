// The Library toolbar's sort modes and the search predicate both grids
// share (design §5, §6). Pure.
import type { InstalledGame } from '../api';

export const LIBRARY_SORTS = ['played', 'installed', 'title', 'platform'] as const;
export type LibrarySort = (typeof LIBRARY_SORTS)[number];

const LABELS: Record<LibrarySort, string> = {
  played: 'Recently played',
  installed: 'Recently installed',
  title: 'Title',
  platform: 'Platform',
};

export function normalizeSort(raw: string): LibrarySort {
  const trimmed = raw.trim();
  return (LIBRARY_SORTS as readonly string[]).includes(trimmed)
    ? (trimmed as LibrarySort)
    : 'title';
}

export function sortLabel(sort: LibrarySort): string {
  return LABELS[sort];
}

const fold = (value: string) => value.trim().toLowerCase();

function byTitle(a: InstalledGame, b: InstalledGame): number {
  const ta = fold(a.title);
  const tb = fold(b.title);
  if (ta !== tb) return ta < tb ? -1 : 1;
  return 0;
}

/**
 * A stable, non-mutating sort. Every mode falls back to title so the grid
 * never reorders itself between renders over rows that tie — two games
 * installed in the same second, or two never played.
 */
export function sortGames(rows: InstalledGame[], sort: LibrarySort): InstalledGame[] {
  const out = rows.slice();
  if (sort === 'title') {
    out.sort(byTitle);
  } else if (sort === 'platform') {
    out.sort((a, b) => {
      const pa = fold(a.platform);
      const pb = fold(b.platform);
      if (pa !== pb) return pa < pb ? -1 : 1;
      return byTitle(a, b);
    });
  } else if (sort === 'installed') {
    out.sort((a, b) => b.installed_at - a.installed_at || byTitle(a, b));
  } else {
    out.sort((a, b) => b.last_played_at - a.last_played_at || byTitle(a, b));
  }
  return out;
}

/** Design §5 / §6: "search (title contains)", case-insensitive. A blank
 *  query matches everything so the grid is never empty just from focus. */
export function titleContains(title: string, query: string): boolean {
  const needle = fold(query);
  if (needle === '') return true;
  return title.toLowerCase().includes(needle);
}
