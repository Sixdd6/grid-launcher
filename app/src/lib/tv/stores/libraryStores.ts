// Store games in the Library's own rows (TVStores README § Library: "In All
// games, Recent and Updates the store games carry a store tag"; prototype
// `tvstores.js` second `libPaint`), and a drive's GOG game as a store game.
// Pure.
import type { DriveStoreGame, InstalledGame, StoreGame } from '../../api';
import { matchesRail } from '../../library/rail';

/** The registry row of a store game (its last played time lives there). */
export function storeRowOf(rows: readonly InstalledGame[], game: StoreGame): InstalledGame | null {
  return rows.find((r) => r.source === game.source && r.store_id === game.store_id) ?? null;
}

/**
 * The installed store games All games, Recent or Updates shows after the
 * RomM ones, titles A–Z: every one in All games, those played lately in
 * Recent (the desktop's rule, read from the game's registry row), those with
 * an update in Updates.
 */
export function mixedStoreGames(
  games: readonly StoreGame[],
  rows: readonly InstalledGame[],
  key: 'all' | 'recent' | 'updates',
  nowSeconds: number,
): StoreGame[] {
  const out = games.filter((g) => {
    if (!g.installed) return false;
    if (key === 'all') return true;
    if (key === 'updates') return g.update_version !== '';
    const row = storeRowOf(rows, g);
    return row !== null && matchesRail(row, 'recent', new Set(), nowSeconds);
  });
  return out.sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
}

/** A drive's GOG game GRID has no owned record of (signed out): its page from the drive's record. */
export function storeGameFromDrive(dg: DriveStoreGame): StoreGame {
  const id = dg.key.includes(':') ? dg.key.slice(dg.key.indexOf(':') + 1) : dg.key;
  return {
    key: dg.key,
    source: 'gog',
    store_id: id,
    title: dg.name,
    installed: dg.installed,
    cover: dg.cover,
    screenshots: [],
    description: dg.summary,
    developer: '',
    genres: [],
    year: '',
    platforms: [dg.platform],
    installed_platform: dg.installed ? dg.platform : null,
    download_bytes: dg.size,
    disk_bytes: dg.size,
    version: dg.version,
    update_version: '',
    origin: dg.installed ? 'drive' : null,
    not_supported: '',
    dlc: [],
    cloud: 'unknown',
    last_sync: 0,
    install_dir: '',
    compat_tool: '',
  };
}
