// GOG games on backup drives (GameStores README, "GOG games on backup
// drives"; prototype stores_bk.js): the backup dialog's rows from
// `plan_store_backup`, the drive grid's GOG covers and counts. Pure.
import type { DriveStoreGame, StoreBackupPlan } from '../api';
import { plural, sizeGb } from './text';

export const STORE_BACKUP_LEDE =
  'GRID writes each GOG game to the drive’s gridpack folder: its installer files when GRID has them, else a copy of the installed game. Saves always go with it. Nothing downloads, and no GOG login is needed.';
export const STORE_BACKUP_COVERS_NOTE =
  'Covers and the store’s details go too. GRID adds no emulator or Proton for GOG games; the computer that installs a Windows game uses its own compatibility tool.';
export const WINDOWS_PREFIX_NOTE = 'Windows games are copied without their Wine prefix. The save folders GOG reports are copied from it.';

/** "Backup Ashen Tide", "Backup 2 games", "Backup GOG · 3 games". */
export function storeBackupTitle(subject: { kind: 'game'; name: string } | { kind: 'games'; count: number } | { kind: 'all'; count: number }): string {
  if (subject.kind === 'game') return `Backup ${subject.name}`;
  if (subject.kind === 'all') return `Backup GOG · ${plural(subject.count, 'game')}`;
  return `Backup ${plural(subject.count, 'game')}`;
}

export type StoreBackupRow = { key: string; name: string; tag: 'Installer files' | 'Installed copy'; icon: 'download' | 'drive'; note: string };

export type StoreBackupView = {
  rows: StoreBackupRow[];
  windowsNote: boolean;
  /** "1 game is already on Couch kit and will be skipped: Ashen Tide." */
  skipped: string;
  /** Refusals, in the warning colour. */
  refusals: string[];
  /** "24.5 GB to write · nothing to download" / "Nothing new to write". */
  footNote: string;
  button: string;
  canStart: boolean;
  /** The drive radio's free line: "412 GB free" or "Needs … · … free". */
  over: boolean;
};

export function storeBackupView(plan: StoreBackupPlan, packName: string): StoreBackupView {
  const writes = plan.rows.filter((r) => !r.already && r.refused.trim() === '');
  const skipped = plan.rows.filter((r) => r.already);
  const n = writes.length;
  const over = !plan.fits && n > 0;
  return {
    rows: writes.map((r) => ({
      key: r.key,
      name: r.name,
      tag: r.layout === 'installer' ? 'Installer files' : 'Installed copy',
      icon: r.layout === 'installer' ? 'download' : 'drive',
      note: `GOG · ${r.detail}`,
    })),
    windowsNote: writes.some((r) => r.windows_prefix_note),
    skipped:
      skipped.length === 0
        ? ''
        : `${plural(skipped.length, 'game is', 'games are')} already on ${packName} and will be skipped: ${skipped.map((r) => r.name).join(', ')}.`,
    refusals: plan.rows.filter((r) => r.refused.trim() !== '').map((r) => r.refused.trim()),
    footNote: n > 0 ? `${sizeGb(plan.bytes_to_write)} to write · nothing to download` : 'Nothing new to write',
    button: over ? 'Not enough space' : n > 0 ? `Backup ${plural(n, 'game')}` : 'Nothing to backup',
    canStart: n > 0 && !over,
    over,
  };
}

/** The drive radio's space line for a plan. */
export function driveFreeLine(plan: StoreBackupPlan): string {
  return plan.fits ? `${sizeGb(plan.free_bytes)} free` : `Needs ${sizeGb(plan.bytes_to_write)} · ${sizeGb(plan.free_bytes)} free`;
}

/** The drive header's chip: "GOG: 2 games". */
export function driveGogChip(games: readonly DriveStoreGame[]): string | null {
  return games.length === 0 ? null : `GOG: ${plural(games.length, 'game')}`;
}

/** A drive GOG cover's hover action: Play when installed here, else Install. */
export function driveStorePrimary(g: DriveStoreGame, busy: boolean): { label: string; act: 'play' | 'install' | 'details' } {
  if (g.installed) return { label: 'Play', act: 'play' };
  if (busy) return { label: 'Installing…', act: 'details' };
  return { label: 'Install', act: 'install' };
}

/** A drive's GOG games, filtered by the search and in title order, after its RomM games. */
export function driveStoreGrid(games: readonly DriveStoreGame[], search: string): DriveStoreGame[] {
  const q = search.trim().toLowerCase();
  return games
    .filter((g) => q === '' || g.name.toLowerCase().includes(q))
    .slice()
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }));
}
