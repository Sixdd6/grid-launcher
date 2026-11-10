// Library › Backup drives (GameBackups README): the drive header's text and
// chips, the drive grid's sorts and its empty and search states. Pure —
// Library.svelte reads the drive and its games and renders these.
import type { DriveGame, DriveInfo } from '../api';
import { sizeText } from '../backups/dialog';
import { DRIVE_EMPTY_TEXT } from './rail';
import { titleContains } from './sort';

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** The drive's name for people: its volume label, else where it is mounted. */
export function driveLabel(drive: Pick<DriveInfo, 'label' | 'mount'>): string {
  return drive.label.trim() || drive.mount;
}

/** The header's count line: "8 games · 13.3 GB · on SAMSUNG T7". */
export function driveCountLine(drive: DriveInfo): string {
  const pack = drive.pack;
  const games = pack?.game_count ?? 0;
  const bytes = pack?.gridpack_bytes ?? 0;
  return `${plural(games, 'game')} · ${sizeText(bytes)} · on ${driveLabel(drive)}`;
}

/**
 * The header's chips: "Emulators: DuckStation, PCSX2" and "Firmware:
 * PlayStation", each "none" when the pack has nothing. An emulator carried
 * for two computers is one name.
 */
export function driveChips(drive: DriveInfo): { emulators: string; firmware: string } {
  const pack = drive.pack;
  const names: string[] = [];
  for (const e of pack?.emulators ?? []) {
    const name = e.name.trim();
    if (name !== '' && !names.includes(name)) names.push(name);
  }
  const firmware = (pack?.firmware ?? []).map((f) => f.trim()).filter((f, i, all) => f !== '' && all.indexOf(f) === i);
  return {
    emulators: `Emulators: ${names.length ? names.join(', ') : 'none'}`,
    firmware: `Firmware: ${firmware.length ? firmware.join(', ') : 'none'}`,
  };
}

export const DRIVE_SORTS = ['title', 'platform', 'added', 'size'] as const;
export type DriveSort = (typeof DRIVE_SORTS)[number];

const LABELS: Record<DriveSort, string> = {
  title: 'Title',
  platform: 'Platform',
  added: 'Recently backed up',
  size: 'Largest first',
};

export function driveSortLabel(sort: DriveSort): string {
  return LABELS[sort];
}

export function normalizeDriveSort(raw: string): DriveSort {
  const trimmed = raw.trim();
  return (DRIVE_SORTS as readonly string[]).includes(trimmed) ? (trimmed as DriveSort) : 'title';
}

const fold = (value: string) => value.trim().toLowerCase();

function byTitle(a: DriveGame, b: DriveGame): number {
  const ta = fold(a.name);
  const tb = fold(b.name);
  return ta === tb ? 0 : ta < tb ? -1 : 1;
}

/** The drive's games that match `search`, in `sort` order. Every mode falls back to title. Non-mutating. */
export function driveGrid(games: readonly DriveGame[], search: string, sort: DriveSort): DriveGame[] {
  const out = games.filter((g) => titleContains(g.name, search));
  if (sort === 'title') out.sort(byTitle);
  else if (sort === 'platform')
    out.sort((a, b) => {
      const pa = fold(a.platform_name);
      const pb = fold(b.platform_name);
      return pa === pb ? byTitle(a, b) : pa < pb ? -1 : 1;
    });
  // RFC 3339 UTC strings compare in time order.
  else if (sort === 'added') out.sort((a, b) => (a.added === b.added ? byTitle(a, b) : a.added < b.added ? 1 : -1));
  else out.sort((a, b) => b.size - a.size || byTitle(a, b));
  return out;
}

/** What the drive grid says with no cards: a search that matches nothing, else the empty drive. */
export function driveEmptyText(search: string): string {
  const q = search.trim();
  return q === '' ? DRIVE_EMPTY_TEXT : `No games on this drive match “${q}”`;
}

/** A drive game's identity on the grid: one pack may hold the same rom id from two servers. */
export function driveGameKey(game: Pick<DriveGame, 'server' | 'rom_id'>): string {
  return `${game.server}\u0000${game.rom_id}`;
}

/** "Removed 3 games from Couch kit" / "Removed Cinder Peaks from Couch kit". */
export function removedFromDriveText(names: readonly string[], packName: string): string {
  const what = names.length === 1 ? names[0] : plural(names.length, 'game');
  return `Removed ${what} from ${packName}`;
}

/** The select bar's install button: "Install 3 games", or "Install" before anything is picked. */
export function installSelectionLabel(count: number): string {
  return count === 0 ? 'Install' : `Install ${plural(count, 'game')}`;
}
