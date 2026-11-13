// The Library rail (design §5, D-UI-2): All games, Recent, Updates, then
// the installed platforms with counts, then (GameBackups README) one row per
// connected backup drive under "BACKUP DRIVES". Pure — Library.svelte renders these
// entries and asks `matchesRail` which rows an entry keeps.
import type { DriveInfo, InstalledGame } from '../api';
import { isHiddenLibraryPlatform } from '../library';

/** Design §5: "Recent (played in the last 30 days)", in whole seconds. */
export const RECENT_WINDOW_SECONDS = 30 * 24 * 60 * 60;

export type RailKey =
  | 'all'
  | 'recent'
  | 'updates'
  | `platform:${string}`
  | `drive:${string}`
  // A store group's row (GameStores README): `store:gog:installed`, `store:gog:available`.
  | `store:${string}`;

export type RailEntry = {
  key: RailKey;
  /** The §11 `library-rail-<key>` test id for this entry. */
  testId: string;
  label: string;
  count: number;
  /** A drive row's tooltip: the volume label. */
  title?: string;
};

/** The rail key of the drive with `key` (its mount path, or its drive letter on Windows). */
export function driveRailKey(key: string): RailKey {
  return `drive:${key}`;
}

/** The drive key a `drive:` rail key names, or `null` for any other key. */
export function driveKeyOf(key: RailKey): string | null {
  return key.startsWith('drive:') ? key.slice('drive:'.length) : null;
}

/**
 * A platform name reduced to the id-safe token the rail's test id carries.
 * Runs of anything that is not a letter or digit collapse to one dash, and
 * leading/trailing dashes are dropped, so "Sega CD / Mega-CD" and
 * "Sega CD  Mega CD" cannot produce two different rail entries for one name.
 */
export function platformSlug(platform: string): string {
  return platform
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

const platformKey = (platform: string): RailKey => `platform:${platformSlug(platform)}`;

function isRecent(row: InstalledGame, nowSeconds: number): boolean {
  // A zero stamp is "never launched", not "launched at the epoch".
  if (row.last_played_at <= 0) return false;
  return nowSeconds - row.last_played_at <= RECENT_WINDOW_SECONDS;
}

function hasUpdate(row: InstalledGame, updateRomIds: ReadonlySet<number>): boolean {
  return row.rom_id !== null && updateRomIds.has(row.rom_id);
}

/** Whether `row` belongs under `key`. `all` keeps everything. */
export function matchesRail(
  row: InstalledGame,
  key: RailKey,
  updateRomIds: ReadonlySet<number>,
  nowSeconds: number,
): boolean {
  if (key === 'all') return true;
  if (key === 'recent') return isRecent(row, nowSeconds);
  if (key === 'updates') return hasUpdate(row, updateRomIds);
  // A drive's grid is the drive's own games, not installed rows; a store
  // view's grid is the store's games.
  if (driveKeyOf(key) !== null || key.startsWith('store:')) return false;
  return platformKey(row.platform) === key;
}

/**
 * The rail, in design §5's order. The synthetic "Emulators" platform is
 * excluded everywhere, counts included, exactly as the grid excludes it
 * (`isHiddenLibraryPlatform`, ported from game_views.py:297-311).
 * `drives` (the store's `withPack`: connected drives with a readable pack)
 * add their rows last, in the order given.
 */
export function railEntries(
  rows: InstalledGame[],
  updateRomIds: ReadonlySet<number>,
  nowSeconds: number,
  drives: readonly DriveInfo[] = [],
): RailEntry[] {
  const visible = rows.filter((row) => !isHiddenLibraryPlatform(row.platform));

  const platforms = new Map<RailKey, { label: string; count: number }>();
  for (const row of visible) {
    const key = platformKey(row.platform);
    const existing = platforms.get(key);
    if (existing) existing.count += 1;
    else platforms.set(key, { label: row.platform.trim(), count: 1 });
  }

  const entries: RailEntry[] = [
    { key: 'all', testId: 'library-rail-all', label: 'All games', count: visible.length },
    {
      key: 'recent',
      testId: 'library-rail-recent',
      label: 'Recent',
      count: visible.filter((row) => isRecent(row, nowSeconds)).length,
    },
    {
      key: 'updates',
      testId: 'library-rail-updates',
      label: 'Updates',
      count: visible.filter((row) => hasUpdate(row, updateRomIds)).length,
    },
  ];

  const sorted = [...platforms.entries()].sort((a, b) =>
    a[1].label.toLowerCase() < b[1].label.toLowerCase() ? -1 : 1,
  );
  for (const [key, value] of sorted) {
    entries.push({
      key,
      testId: `library-rail-platform-${key.slice('platform:'.length)}`,
      label: value.label,
      count: value.count,
    });
  }
  entries.push(...driveRailEntries(drives));
  return entries;
}

/**
 * Library › Backup drives: one row per connected drive with a pack, the
 * pack name and its game count, the volume label as the tooltip. A drive
 * without a readable pack has no row.
 */
export function driveRailEntries(drives: readonly DriveInfo[]): RailEntry[] {
  return drives.flatMap((drive) =>
    drive.pack === null
      ? []
      : [
          {
            key: driveRailKey(drive.key),
            testId: `library-rail-drive-${platformSlug(drive.key) || 'drive'}`,
            label: drive.pack.name,
            count: drive.pack.game_count,
            title: drive.label.trim() || drive.mount,
          },
        ],
  );
}

/** The entry for `key`, falling back to `all` when the key has gone away
 *  (the last game on a platform was uninstalled while it was selected). */
export function entryForKey(entries: RailEntry[], key: RailKey): RailEntry {
  return entries.find((entry) => entry.key === key) ?? entries[0];
}

/** An empty drive's grid (GameBackups prototype). */
export const DRIVE_EMPTY_TEXT = 'Nothing on this drive yet. Backup games from Server or Library.';

/** Design §5's empty-state text, one line per rail entry, verbatim. */
export function emptyText(entry: RailEntry): string {
  if (entry.key === 'recent') return 'Nothing played in the last 30 days';
  if (entry.key === 'updates') return 'Everything is up to date';
  if (entry.key === 'all') return 'No games installed';
  if (driveKeyOf(entry.key) !== null) return DRIVE_EMPTY_TEXT;
  return `No games installed for ${entry.label}`;
}
