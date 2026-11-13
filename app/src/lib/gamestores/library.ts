// Store games in the Library's own rows (GameStores README, designer
// decision 4): All games, Recent and Updates include the installed store
// games, each with its store's tag; platform rows don't. Pure.
import type { InstalledGame, StoreGame } from '../api';
import { storeName } from './marks';
import { sizeGb } from './text';

/** Design §5: "Recent (played in the last 30 days)", in seconds. */
const RECENT_WINDOW_SECONDS = 30 * 24 * 60 * 60;

/** The registry row of an installed store game (its play stamps live there), or `undefined`. */
export function rowOf(rows: readonly InstalledGame[], game: StoreGame): InstalledGame | undefined {
  return rows.find((r) => r.source === game.source && r.store_id === game.store_id);
}

export type MixKey = 'all' | 'recent' | 'updates';

function inKey(game: StoreGame, row: InstalledGame | undefined, key: MixKey, now: number): boolean {
  if (key === 'all') return true;
  if (key === 'updates') return game.update_version !== '';
  const at = row?.last_played_at ?? 0;
  return at > 0 && now - at <= RECENT_WINDOW_SECONDS;
}

/** The installed store games a mixed row shows, title order, after the RomM games. */
export function mixedStoreGames(
  games: readonly StoreGame[],
  rows: readonly InstalledGame[],
  key: string,
  search: string,
  now: number,
): StoreGame[] {
  if (key !== 'all' && key !== 'recent' && key !== 'updates') return [];
  const q = search.trim().toLowerCase();
  return games
    .filter((g) => g.installed && inKey(g, rowOf(rows, g), key, now))
    .filter((g) => q === '' || g.title.toLowerCase().includes(q) || storeName(g.source).toLowerCase().includes(q))
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
}

/** How many store games each mixed row adds to its rail count. */
export function mixedCounts(games: readonly StoreGame[], rows: readonly InstalledGame[], now: number): Record<MixKey, number> {
  const inst = games.filter((g) => g.installed);
  return {
    all: inst.length,
    recent: inst.filter((g) => inKey(g, rowOf(rows, g), 'recent', now)).length,
    updates: inst.filter((g) => g.update_version !== '').length,
  };
}

/** A store view's sort: the Library's modes, read from each game's registry row. */
export function sortStoreView(games: readonly StoreGame[], sort: string, rows: readonly InstalledGame[]): StoreGame[] {
  const byTitle = (a: StoreGame, b: StoreGame) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' });
  const out = [...games];
  if (sort === 'played') out.sort((a, b) => (rowOf(rows, b)?.last_played_at ?? 0) - (rowOf(rows, a)?.last_played_at ?? 0) || byTitle(a, b));
  else if (sort === 'installed') out.sort((a, b) => (rowOf(rows, b)?.installed_at ?? 0) - (rowOf(rows, a)?.installed_at ?? 0) || byTitle(a, b));
  else out.sort(byTitle);
  return out;
}

/** "2 selected · 24.5 GB to write" sums the installed sizes (the dialog plans the real bytes). */
export function selectedBytes(games: readonly StoreGame[], keys: readonly string[]): number {
  return games.filter((g) => keys.includes(g.key)).reduce((t, g) => t + g.disk_bytes, 0);
}

/** The select bar's second line in GOG › Installed: "24.5 GB to write". */
export function selectedToWrite(games: readonly StoreGame[], keys: readonly string[]): string {
  return `${sizeGb(selectedBytes(games, keys))} to write`;
}
