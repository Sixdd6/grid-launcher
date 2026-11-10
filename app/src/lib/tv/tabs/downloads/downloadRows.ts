// The Downloads tab's words and rules (checklist items 209–224; tvmode.js
// `TABS.downloads`, `ACT.dlrow`; ruling 8). Pure: TvDownloads draws them over
// the downloads store and runs the actions through `downloads/actions.ts`.
import type { DownloadEntry, MovePhase } from '../../../api';
import { actionFor, copiesFromDrive, entryDetail, etaText, formatSize, kindLabel, percent } from '../../../downloads/format';
import { groupBySegment, type Segment } from '../../../downloads/segments';
import { CANCEL_EXTRACTING, cancelConfirm } from '../../pages/details';

export type TvSegment = { seg: Segment; name: string; empty: string; rows: DownloadEntry[] };

const NAMES: Record<Segment, string> = { active: 'Active', queued: 'Queued', completed: 'Completed' };

/** The three panels in order, each with its rows (items 209–211): Completed
 *  holds finished, failed and cancelled rows. */
export function tvSegments(entries: DownloadEntry[]): TvSegment[] {
  const groups = groupBySegment(entries);
  return (['active', 'queued', 'completed'] as const).map((seg) => ({
    seg,
    name: NAMES[seg],
    empty: `Nothing ${NAMES[seg].toLowerCase()}.`,
    rows: groups[seg],
  }));
}

/** The kind badge after the title (item 212): the desktop's labels, with a
 *  library move's shortened to "Library" as on the prototype. */
export function kindBadge(e: DownloadEntry): string {
  return e.kind === 'library_move' ? 'Library' : kindLabel(e.kind);
}

function isBackup(e: DownloadEntry): boolean {
  return e.job === 'backup';
}

/** The status after the platform (items 212–214): the prototype's dotted
 *  wording for an install, the Game backups card's for a backup, the
 *  desktop's for a library move. */
export function statusText(e: DownloadEntry): string {
  if (isBackup(e) || e.kind === 'library_move') return entryDetail(e).replace(/ • /g, ' · ');
  const drive = copiesFromDrive(e) ? e.drive_label : '';
  switch (e.status) {
    case 'queued':
      return drive ? `Queued · from ${drive}` : 'Queued';
    case 'downloading': {
      const parts = [drive ? `Copying from ${drive}` : 'Downloading'];
      if (e.total_bytes > 0) parts.push(`${percent(e.downloaded_bytes, e.total_bytes)}%`);
      if (!drive && e.speed_bps > 0) parts.push(`${formatSize(e.speed_bps)}/s`);
      const eta = drive ? '' : etaText(e);
      if (eta) parts.push(eta);
      return parts.join(' · ');
    }
    case 'installing':
      return e.install_total_bytes > 0 ? `Installing · ${percent(e.install_processed_bytes, e.install_total_bytes)}%` : 'Installing…';
    case 'cancelling':
      return 'Cancelling…';
    case 'completed':
      return e.downloaded_bytes > 0 ? `Completed · ${formatSize(e.downloaded_bytes)}` : 'Completed';
    case 'failed':
      return `Failed · ${e.error.trim() || 'Unknown error'}`;
    case 'cancelled':
      return 'Cancelled';
    default:
      return 'Unknown';
  }
}

/** "<Platform> · <status>" (item 212); a row with no platform shows the status alone. */
export function statusLine(e: DownloadEntry): string {
  const p = e.platform.trim();
  return p ? `${p} · ${statusText(e)}` : statusText(e);
}

/** A failed row's status is in the danger colour (item 214). */
export function isFailed(e: DownloadEntry): boolean {
  return e.status === 'failed';
}

/** The progress bar's fill while active (item 212), else null (no bar). */
export function progress(e: DownloadEntry): number | null {
  switch (e.status) {
    case 'downloading':
    case 'cancelling':
      return percent(e.downloaded_bytes, e.total_bytes);
    case 'installing':
      return percent(e.install_processed_bytes, e.install_total_bytes);
    default:
      return null;
  }
}

/** The speed beside the sparkline while bytes move (item 212). */
export function speedText(e: DownloadEntry): string {
  if (e.kind === 'library_move') return '';
  if ((e.status === 'downloading' || e.status === 'cancelling') && e.speed_bps > 0) return `${formatSize(e.speed_bps)}/s`;
  return '';
}

export type RowActionKind = 'cancel' | 'retry' | 'dismiss' | 'open';
export type RowAction = { kind: RowActionKind; label: string; danger?: boolean };

/** A game the row installs or backs up, which "Open <title>" opens (item 217). */
export function opensGame(e: DownloadEntry): boolean {
  if (e.rom_id <= 0) return false;
  if (e.job === 'game') return true;
  return e.kind === 'backup_game' || e.kind === 'backup_game_local';
}

/**
 * The row's actions (item 217), in the prototype's order: Cancel (danger)
 * while it can be stopped, Retry when failed or cancelled, Dismiss when
 * finished, failed or cancelled, then "Open <title>" for a game. Which of
 * them a row offers follows the desktop's `actionFor` (a firmware row and an
 * extracting install can't be cancelled; a library move's Cancel and Retry
 * are the move's own).
 */
export function rowActions(e: DownloadEntry, movePhase: MovePhase | null): RowAction[] {
  const act = actionFor(e.status, e.kind, movePhase);
  const out: RowAction[] = [];
  if (act === 'cancel' || act === 'retry-cancel') out.push({ kind: 'cancel', label: 'Cancel', danger: true });
  if (act === 'retry-dismiss' || act === 'retry-cancel' || act === 'retry') out.push({ kind: 'retry', label: 'Retry' });
  if (act === 'retry-dismiss' || act === 'dismiss') out.push({ kind: 'dismiss', label: 'Dismiss' });
  if (opensGame(e)) out.push({ kind: 'open', label: `Open ${e.title}` });
  return out;
}

/**
 * The A hint (item 216): "Cancel" while active or queued, "Retry or dismiss"
 * when failed, "Dismiss" when finished or cancelled (tvmode.js `row`). A row
 * that can't be stopped right now says "Details" while it still opens its
 * game, and is disabled (null) with nothing to offer.
 */
export function rowA(e: DownloadEntry, movePhase: MovePhase | null): string | null {
  const acts = rowActions(e, movePhase).map((a) => a.kind);
  if (acts.includes('cancel')) return acts.includes('retry') ? 'Retry or cancel' : 'Cancel';
  if (e.status === 'failed' && acts.includes('retry')) return acts.includes('dismiss') ? 'Retry or dismiss' : 'Retry';
  if (acts.includes('dismiss')) return 'Dismiss';
  if (acts.includes('retry')) return 'Retry';
  // Only "Open <title>" is left (an extracting install): "Details"
  // (consultant decision; checklist 534).
  if (acts.length === 1 && acts[0] === 'open') return 'Details';
  return acts.length ? 'Select' : null;
}

export const FIRMWARE_WHY = 'Installing firmware — this step cannot be cancelled';
export const MOVE_WHY = 'Updating paths — this step cannot be cancelled';

/** Why a row with no actions can't be pressed (shown as a notice on A;
 *  checklist 534): by kind. */
export function rowWhy(e: DownloadEntry): string {
  if (e.kind === 'firmware' || e.job === 'firmware') return FIRMWARE_WHY;
  if (e.kind === 'library_move') return MOVE_WHY;
  return CANCEL_EXTRACTING;
}

/** The action list's line: "<Platform> · <kind>" (item 217). */
export function listSub(e: DownloadEntry): string {
  return [e.platform.trim(), kindBadge(e)].filter((s) => s !== '').join(' · ');
}

export type CancelWords = { title: string; body: string; ok: string };

/**
 * Cancel asks first for every kind of row, with the ruled words (ruling 8):
 * an install (game, update, emulator, firmware), a backup row, the library
 * move. A queued install only leaves the queue. Danger action, Cancel
 * focused; the card that follows is the notifier's ("Cancelled <title>").
 */
export function cancelWords(e: DownloadEntry): CancelWords {
  if (e.kind === 'library_move') {
    return {
      title: 'Cancel the move?',
      body: 'The files copied so far are removed and the library stays where it is.',
      ok: 'Cancel move',
    };
  }
  if (isBackup(e)) {
    const label = e.drive_label.trim() || e.drive_name.trim() || 'the drive';
    return {
      title: `Cancel ${e.title}?`,
      body: `The backup stops and what it wrote to ${label} is removed. You can back it up again later.`,
      ok: 'Cancel backup',
    };
  }
  if (e.status === 'queued') {
    return { title: `Cancel ${e.title}?`, body: 'It leaves the queue. You can install it again later.', ok: 'Cancel install' };
  }
  return cancelConfirm(e.title);
}

/** Y "Clear completed" shows only while Completed has rows (item 222). */
export function hasCompleted(entries: DownloadEntry[]): boolean {
  return entries.some((e) => e.status === 'completed' || e.status === 'failed' || e.status === 'cancelled');
}

/** The rows Y clears (item 222): every finished, failed and cancelled one. */
export function completedIds(entries: DownloadEntry[]): number[] {
  return entries.filter((e) => e.status === 'completed' || e.status === 'failed' || e.status === 'cancelled').map((e) => e.id);
}
