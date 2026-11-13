// A store game's live Downloads row: which row belongs to a game, how far it
// is, and what it is doing. Pure; the covers, Details and Downloads all read
// it (GameStores README, "Store covers" and "Downloads and notifications").
import type { DownloadEntry, DownloadKind } from '../api';

const LIVE = new Set(['queued', 'downloading', 'installing', 'cancelling']);

export function isLive(e: DownloadEntry): boolean {
  return LIVE.has(e.status);
}

/** A row that installs, updates, repairs, adds DLC to or restores a store game. */
export function isStoreJob(e: DownloadEntry): boolean {
  return e.job === 'store';
}

/** The kinds that act on one game (not the store tool's own download). */
const GAME_KINDS: ReadonlySet<DownloadKind> = new Set([
  'store_install',
  'store_update',
  'store_repair',
  'store_dlc',
  'store_installer',
  'store_restore',
]);

/**
 * The live store row for `key` (`gog:1207664643`), or `null`. An active row
 * wins over a queued one; DLC rows count (the cover shows its progress too).
 */
export function liveJobFor(entries: readonly DownloadEntry[], key: string): DownloadEntry | null {
  const mine = entries.filter((e) => isStoreJob(e) && GAME_KINDS.has(e.kind) && e.source_id === key && isLive(e));
  return mine.find((e) => e.status !== 'queued') ?? mine[0] ?? null;
}

/** The live row of a RomM game (an install that may run a GOG installer), or `null`. */
export function liveRommJobFor(entries: readonly DownloadEntry[], romId: number): DownloadEntry | null {
  const mine = entries.filter((e) => e.job === 'game' && e.rom_id === romId && isLive(e));
  return mine.find((e) => e.status !== 'queued') ?? mine[0] ?? null;
}

/** The store tool's live download for `source` (`gog`), or `null`. */
export function liveToolJob(entries: readonly DownloadEntry[], source: string): DownloadEntry | null {
  return (
    entries.find(
      (e) => isStoreJob(e) && e.kind === 'store_tool' && isLive(e) && (e.source_id === source || e.source_id.startsWith(`${source}:`)),
    ) ?? null
  );
}

function pct(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.trunc((done * 100) / total)));
}

/**
 * How far the row is, 0–100: the install half's bytes while it installs,
 * else the download's. A queued row is 0.
 */
export function jobPercent(e: DownloadEntry): number {
  if (e.status === 'queued') return 0;
  if (e.status === 'completed') return 100;
  if (e.status === 'installing' && e.install_total_bytes > 0) return pct(e.install_processed_bytes, e.install_total_bytes);
  if (e.total_bytes > 0) return pct(e.downloaded_bytes, e.total_bytes);
  if (e.install_total_bytes > 0) return pct(e.install_processed_bytes, e.install_total_bytes);
  return 0;
}

/** What a game's live row does, for the words on its cover and in Details. */
export type JobAct = 'install' | 'update' | 'repair' | 'dlc' | 'restore';

export function jobAct(e: DownloadEntry): JobAct {
  switch (e.kind) {
    case 'store_update':
      return 'update';
    case 'store_repair':
      return 'repair';
    case 'store_dlc':
      return 'dlc';
    case 'store_restore':
      return 'restore';
    default:
      return 'install';
  }
}
