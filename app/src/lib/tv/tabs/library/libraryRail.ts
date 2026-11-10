// The TV Library rail (checklist items 176–184, 420; tvmode.js `libRailHTML`,
// `folder`, `railItem`): All games, Recent and Updates with their counts, a
// divider, then two folders, Platforms (each installed platform with its
// count) and Backup drives (one row per connected, set-up drive with its game
// count, or "No Backup drives connected"). Pure, over the desktop's rail
// rules (`library/rail.ts`): the same counts, the same keys.
import type { DriveInfo, InstalledGame } from '../../../api';
import { visibleLibraryGames } from '../../../library';
import { matchesRail, railEntries, type RailEntry, type RailKey } from '../../../library/rail';

export type FolderId = 'plat' | 'drives';

/** Open / closed per folder; both start open and stay as left for the session (item 183). */
export type Folds = Record<FolderId, boolean>;

export const DEFAULT_FOLDS: Folds = { plat: true, drives: true };

export type LibraryRail = {
  /** All games, Recent, Updates. */
  filters: RailEntry[];
  platforms: RailEntry[];
  drives: RailEntry[];
};

/** The rail's rows from the installed registry, the updates and the drives with a pack. */
export function libraryRail(
  rows: InstalledGame[],
  updateRomIds: ReadonlySet<number>,
  nowSeconds: number,
  drives: readonly DriveInfo[],
): LibraryRail {
  const all = railEntries(rows, updateRomIds, nowSeconds, drives);
  return {
    filters: all.slice(0, 3),
    platforms: all.filter((e) => e.key.startsWith('platform:')),
    drives: all.filter((e) => e.key.startsWith('drive:')),
  };
}

/** Every row the rail can select, in order (a vanished key falls back to All games). */
export function railKeys(rail: LibraryRail): RailKey[] {
  return [...rail.filters, ...rail.platforms, ...rail.drives].map((e) => e.key);
}

/** The entry for `key`, or All games when it has gone (an unplugged drive). */
export function railEntry(rail: LibraryRail, key: RailKey): RailEntry {
  return [...rail.filters, ...rail.platforms, ...rail.drives].find((e) => e.key === key) ?? rail.filters[0];
}

/** Which folder holds `key`, or null for All games, Recent and Updates. */
export function folderOf(key: RailKey): FolderId | null {
  if (key.startsWith('platform:')) return 'plat';
  if (key.startsWith('drive:')) return 'drives';
  return null;
}

/** A row's `data-k` (tvmode.js `railItem`: "r-<key>"). */
export function railRowKey(key: RailKey): string {
  return `r-${key}`;
}

/** A folder header's `data-k` (tvmode.js `folder`: "fold-<id>"). */
export function folderKey(id: FolderId): string {
  return `fold-${id}`;
}

/** A folder header's A hint: "Close" while open, "Open" while closed (item 180). */
export function folderHint(open: boolean): string {
  return open ? 'Close' : 'Open';
}

/** The folder names (item 176). */
export const FOLDER_LABELS: Record<FolderId, string> = { plat: 'Platforms', drives: 'Backup drives' };

/** The Backup drives folder with no drive (item 420). */
export const NO_DRIVES = 'No Backup drives connected';

/** What "Backup all N…" on a Library platform row backs up (item 455): its
 *  installed games the server knows (a rom id). N counts these, as the
 *  Backup page does. */
export function platformBackupIds(rows: InstalledGame[], key: RailKey, updateRomIds: ReadonlySet<number>, nowSeconds: number): number[] {
  return visibleLibraryGames(rows).flatMap((r) => (matchesRail(r, key, updateRomIds, nowSeconds) && r.rom_id !== null ? [r.rom_id] : []));
}

/** X on a Library platform row: "Backup all N…", installed games only (item 455). */
export function backupAllLabel(installed: number): string {
  return `Backup all ${installed}…`;
}
