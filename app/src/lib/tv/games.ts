// One game as a TV cover shows it, whichever list it comes from: a server
// game (`GameSummary`, with its installed row when there is one) or an
// installed row of the registry. Pure: the covers, the hero and Search
// everything read these, and the shared stores feed them (tvmode.js
// `cover`, `quickLabel`, `dlOf`).
import type { DownloadEntry, DriveGame, GameSummary, InstalledGame } from '../api';
import { driveAssetUrl } from '../backups/driveArt';
import { shortPlatformName } from '../cards/badges';
import { liveInstallStatus } from '../discover/feed';
import { percent } from '../downloads/format';
import { quickAction, type QuickKind } from '../games/quick';
import { installedKey } from '../library';

export type TvGame = {
  /** Stable for the session: the rom id, else `i:` and the registry key. */
  id: string;
  romId: number | null;
  title: string;
  /** The platform's label (the registry and the defaults know it by it). */
  platform: string;
  platformSlug: string;
  /** The cover a 10rem TV cover shows (the large one when there is one). */
  cover: string | null;
  installed: boolean;
  missing: boolean;
  /** Installed, with a newer copy on the server. */
  update: boolean;
  /** Epoch seconds of the last launch; 0 for never. */
  lastPlayedAt: number;
  /** First release, epoch seconds; null when unknown. */
  releasedAt: number | null;
  /** The installed row's summary, or ''. */
  description: string;
  summary: GameSummary | null;
  row: InstalledGame | null;
  /** Shown from a Backup drive: its key and the drive's record of the game. */
  drive?: { key: string; game: DriveGame };
};

function releaseOf(raw: string): number | null {
  const n = Number(raw);
  if (raw.trim() !== '' && Number.isFinite(n) && n > 0) return n > 1e11 ? Math.floor(n / 1000) : n;
  const t = Date.parse(raw);
  return Number.isNaN(t) ? null : Math.floor(t / 1000);
}

/** A server game; `row` is its installed registry row, when installed. */
export function gameFromSummary(
  game: GameSummary,
  platform: string,
  platformSlug: string,
  row: InstalledGame | null,
  updateIds: ReadonlySet<number>,
): TvGame {
  const installed = row !== null;
  return {
    id: String(game.id),
    romId: game.id,
    title: game.name,
    platform,
    platformSlug,
    cover: game.path_cover_large || game.path_cover_small || null,
    installed,
    missing: installed && row.missing === true,
    update: installed && updateIds.has(game.id),
    lastPlayedAt: row?.last_played_at ?? 0,
    releasedAt: game.first_release_date,
    description: row?.description ?? '',
    summary: game,
    row,
  };
}

/** An installed registry row. */
export function gameFromRow(row: InstalledGame, updateIds: ReadonlySet<number>): TvGame {
  return {
    id: row.rom_id !== null ? String(row.rom_id) : `i:${installedKey(row)}`,
    romId: row.rom_id,
    title: row.title,
    platform: row.platform.trim(),
    platformSlug: row.platform_slug,
    cover: row.cover_large_path || row.cover_small_path || null,
    installed: true,
    missing: row.missing === true,
    update: row.rom_id !== null && updateIds.has(row.rom_id),
    lastPlayedAt: row.last_played_at > 0 ? row.last_played_at : 0,
    releasedAt: releaseOf(row.first_release_date),
    description: row.description,
    summary: null,
    row,
  };
}

/**
 * A game on a Backup drive (Library › Backup drives): its cover from the
 * drive, installed when this computer has it (`row`, or the drive's own
 * record of that).
 */
export function gameFromDrive(
  driveKey: string,
  game: DriveGame,
  row: InstalledGame | null,
  updateIds: ReadonlySet<number>,
): TvGame {
  const installed = row !== null || game.installed;
  return {
    id: `${game.rom_id}-${encodeURIComponent(game.server)}`,
    romId: game.rom_id,
    title: game.name,
    platform: game.platform_name.trim(),
    platformSlug: game.platform,
    cover: driveAssetUrl(driveKey, game.cover),
    installed,
    missing: row?.missing === true,
    update: installed && updateIds.has(game.rom_id),
    lastPlayedAt: row?.last_played_at ?? 0,
    releasedAt: releaseOf(game.release_date),
    description: game.summary,
    summary: null,
    row,
    drive: { key: driveKey, game },
  };
}

/** The platform tag in a mixed list (the desktop's chip text). */
export function platformTag(game: TvGame): string {
  return shortPlatformName(game.platform);
}

/** The live game install for `romId`: queued, or running with its percent. */
export type LiveInstall = { queued: boolean; pct: number };

export function liveInstall(entries: readonly DownloadEntry[], romId: number | null): LiveInstall | null {
  if (romId === null) return null;
  const status = liveInstallStatus(entries, romId);
  if (status === null) return null;
  if (status === 'queued') return { queued: true, pct: 0 };
  const e = entries.find((x) => x.job === 'game' && x.rom_id === romId && x.status === status);
  if (!e) return { queued: false, pct: 0 };
  const pct =
    e.status === 'installing' && e.install_total_bytes > 0
      ? percent(e.install_processed_bytes, e.install_total_bytes)
      : percent(e.downloaded_bytes, e.total_bytes);
  return { queued: false, pct: Math.round(pct) };
}

/** A cover's tag while installing: "Queued" or "<n>%" (item 17). */
export function queueTag(live: LiveInstall | null): string | null {
  if (!live) return null;
  return live.queued ? 'Queued' : `${live.pct}%`;
}

/** What X on a cover can do. */
export type TvQuick = Extract<QuickKind, 'play' | 'install' | 'install-drive'>;

/**
 * X on a cover (tvmode.js `quickLabel`): Play when installed, Install from
 * drive on a drive's cover, Install when not (and a server answers);
 * nothing while it installs, for a missing or unlinked game, or with no
 * server.
 */
export function quickFor(
  game: TvGame,
  live: LiveInstall | null,
  connected: boolean,
): { kind: TvQuick; label: string } | null {
  const q = quickAction({
    installed: game.installed,
    romId: game.romId,
    missing: game.missing,
    live: live ? (live.queued ? 'queued' : 'downloading') : null,
    fromDrive: game.drive !== undefined,
    connected,
  });
  return q.kind === 'play' || q.kind === 'install' || q.kind === 'install-drive' ? { kind: q.kind, label: q.label } : null;
}
