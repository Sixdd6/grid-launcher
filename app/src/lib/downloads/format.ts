// Pure display formatting for the downloads/installs UI. Ported verbatim from
// grid_launcher/library/downloads.py (see docs/porting/03-library-install.md,
// "Display text per status" and "Aggregate status text"). No store imports here
// so this module stays import-cycle-free.
import type { DownloadEntry, DownloadKind, DownloadStatus, MovePhase } from '../api';

const SIZE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

export function formatSize(bytes: number): string {
  let size = Math.max(0, bytes);
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < SIZE_UNITS.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const precision = unitIndex === 0 ? 0 : 1;
  return `${size.toFixed(precision)} ${SIZE_UNITS[unitIndex]}`;
}

export function percent(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.trunc((done * 100) / total)));
}

export function entryDetail(e: DownloadEntry): string {
  if (e.kind === 'library_move') return moveDetail(e);
  if (e.job === 'backup') return backupDetail(e);
  // An install from a backup drive reads the drive, not the network.
  const fetching = copiesFromDrive(e) ? `Copying from ${e.drive_label}` : 'Downloading';
  switch (e.status) {
    case 'queued':
      return 'Queued';
    case 'downloading':
      if (e.total_bytes > 0) {
        return (
          `${fetching} ${percent(e.downloaded_bytes, e.total_bytes)}% • ` +
          `${formatSize(e.downloaded_bytes)} / ${formatSize(e.total_bytes)} • ${formatSize(e.speed_bps)}/s`
        );
      }
      return `${fetching} • ${formatSize(e.downloaded_bytes)} • ${formatSize(e.speed_bps)}/s`;
    case 'installing':
      if (e.install_total_bytes > 0) {
        return (
          `Installing ${percent(e.install_processed_bytes, e.install_total_bytes)}% • ` +
          `${formatSize(e.install_processed_bytes)} / ${formatSize(e.install_total_bytes)}`
        );
      }
      return 'Installing...';
    case 'cancelling':
      return 'Cancelling...';
    case 'completed':
      return `Completed • ${e.downloaded_bytes > 0 ? formatSize(e.downloaded_bytes) : 'Unknown size'}`;
    case 'failed':
      return `Failed • ${e.error.trim() || 'Unknown error'}`;
    case 'cancelled':
      return 'Cancelled';
    default:
      return 'Unknown';
  }
}

/** A row that installs from a backup drive (a game, or the emulator or
 *  Proton it brings): `drive_label` names the drive it copies from. */
export function copiesFromDrive(e: DownloadEntry): boolean {
  return e.job !== 'backup' && e.drive_label.trim() !== '';
}

/** The pack name a backup row writes to, else the volume label. */
function packName(e: DownloadEntry): string {
  return e.drive_name.trim() || e.drive_label.trim() || 'the drive';
}

/** The volume label a backup row writes to, else the pack name. */
function volumeLabel(e: DownloadEntry): string {
  return e.drive_label.trim() || e.drive_name.trim() || 'the drive';
}

/**
 * A backup row's line (GameBackups design, "Backups run in Downloads"):
 * `downloading` fetches from the server into staging, `installing` writes
 * to the drive. The summary row stands for the waiting rows of a removed
 * drive; its title is already "13 more items" (or the one item's title).
 */
function backupDetail(e: DownloadEntry): string {
  if (e.kind === 'backup_cancelled') {
    const reason = e.error.trim();
    return `Cancelled${reason === '' ? '' : ` — ${reason}`} · back up again to finish`;
  }
  switch (e.status) {
    case 'queued':
      return `Waiting · to ${packName(e)}`;
    case 'downloading':
      return e.total_bytes > 0
        ? `Downloading from server ${percent(e.downloaded_bytes, e.total_bytes)}%`
        : 'Downloading from server';
    case 'installing':
      return e.install_total_bytes > 0
        ? `Writing to ${volumeLabel(e)} ${percent(e.install_processed_bytes, e.install_total_bytes)}%`
        : `Writing to ${volumeLabel(e)}`;
    case 'cancelling':
      return 'Cancelling...';
    case 'completed':
      return `Written to ${packName(e)}`;
    case 'failed':
      return `Failed — ${e.error.trim() || 'Unknown error'}`;
    case 'cancelled':
      return 'Cancelled';
    default:
      return 'Unknown';
  }
}

/**
 * The library move row's line: the backend's detail while it runs
 * (`games/ · 1.2 of 5.6 GB` while copying, `Updating paths…` after), and a
 * plain outcome once it ends.
 */
function moveDetail(e: DownloadEntry): string {
  const detail = e.detail.trim();
  switch (e.status) {
    case 'downloading':
      return detail === '' ? 'Copying…' : `Copying • ${detail}`;
    case 'installing':
    case 'cancelling':
    case 'queued':
      return detail === '' ? 'Finishing…' : detail;
    case 'completed':
      return 'Completed • The library has moved';
    case 'cancelled':
      return 'Cancelled • The library did not move';
    case 'failed':
      return `Failed • ${e.error.trim() || 'Unknown error'}`;
    default:
      return 'Unknown';
  }
}

const LIVE_STATUSES: DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

export function aggregate(all: DownloadEntry[]): string {
  const live = all.filter((e) => LIVE_STATUSES.includes(e.status));
  if (live.length === 0) return '';
  // The library move is not a download: it never counts, and alone it
  // names itself.
  if (live.every((e) => e.kind === 'library_move')) return 'Moving the library';
  const entries = all.filter((e) => e.kind !== 'library_move');

  const queuedCount = entries.filter((e) => e.status === 'queued').length;
  const activeDownloadCount = entries.filter(
    (e) => e.status === 'downloading' || e.status === 'cancelling'
  ).length;
  const installFinalizeInProgress = entries.some((e) => e.status === 'installing');

  const queuedSuffix =
    queuedCount > 0 ? ` (${queuedCount} queued download${queuedCount !== 1 ? 's' : ''})` : '';

  if (installFinalizeInProgress && activeDownloadCount === 0) {
    return `Installing 1 game${queuedSuffix}`;
  }
  return `${activeDownloadCount} active download${activeDownloadCount !== 1 ? 's' : ''}${queuedSuffix}`;
}

/**
 * The drawer row's small kind badge (task-17-brief.md): blank for the base
 * "just a game" kind (no badge renders — Downloads.svelte hides the span
 * entirely when this is empty), and a short label for every install-specials
 * kind so the drawer distinguishes them from ordinary game rows at a glance.
 */
export function kindLabel(kind: DownloadKind): string {
  switch (kind) {
    case 'base':
      return '';
    case 'ps4_content':
    case 'xbox360_content':
      return 'Content';
    case 'native_update':
    case 'update':
      return 'Update';
    case 'emulator':
      return 'Emulator';
    case 'compat_tool':
      return 'Compat tool';
    case 'firmware':
      return 'Firmware';
    case 'library_move':
      return 'Library move';
    case 'backup_game':
    case 'backup_emulator':
    case 'backup_firmware':
    case 'backup_cancelled':
      return 'Backup';
    default:
      return '';
  }
}

function isBackupKind(kind: DownloadKind): boolean {
  return kind === 'backup_game' || kind === 'backup_emulator' || kind === 'backup_firmware';
}

export type DownloadAction =
  | 'cancel'
  | 'installing'
  | 'retry-dismiss'
  | 'retry-cancel'
  | 'retry'
  | 'dismiss'
  | 'none';

/**
 * Which buttons a drawer row offers.
 *
 * A `firmware` row is the background firmware installer's external entry: it
 * owns no queue slot, so `cancel_install` cannot stop it and `retry_install`
 * returns without doing anything (`JobKey::External`). It therefore gets no
 * button at all while it is live, and only `Dismiss` once it is terminal —
 * never a Cancel or Retry that would do nothing. Every other kind keeps the
 * behavior it had.
 *
 * The `library_move` row's Cancel and Retry are the move's own commands. It
 * offers Cancel only while it copies (`downloading`); committing and
 * cleaning up (`installing`) cannot be stopped. A failed move whose record
 * still waits (`movePhase` from `library_move_status`) offers Retry, plus
 * Cancel while it is still in the copy; with no move waiting, only Dismiss.
 */
export function actionFor(
  status: DownloadStatus,
  kind: DownloadKind = 'base',
  movePhase: MovePhase | null = null,
): DownloadAction {
  if (kind === 'library_move') {
    if (status === 'downloading') return 'cancel';
    if (LIVE_STATUSES.includes(status)) return 'none';
    if (status === 'failed' && movePhase !== null) return movePhase === 'copy' ? 'retry-cancel' : 'retry';
    return 'dismiss';
  }
  if (kind === 'firmware') {
    return LIVE_STATUSES.includes(status) ? 'none' : 'dismiss';
  }
  // The removed drive's summary row is never retried: back up again.
  if (kind === 'backup_cancelled') return 'dismiss';
  // A backup row's write to the drive can be stopped (its writer checks a
  // flag per chunk and removes its temporary file), unlike an extraction.
  if (isBackupKind(kind) && status === 'installing') return 'cancel';
  switch (status) {
    case 'queued':
    case 'downloading':
    case 'cancelling':
      return 'cancel';
    case 'installing':
      return 'installing';
    case 'failed':
    case 'cancelled':
      return 'retry-dismiss';
    default:
      return 'dismiss';
  }
}

/**
 * "The current transfer": the first downloading entry, else the first
 * installing one, else the first entry in any other live state — the same
 * precedence the old drawer footer's progress bar used. `null` when nothing
 * is live. The strip's line and its sparkline both key off this.
 */
export function currentTransfer(entries: DownloadEntry[]): DownloadEntry | null {
  const live = entries.filter((e) => LIVE_STATUSES.includes(e.status));
  if (live.length === 0) return null;
  return (
    live.find((e) => e.status === 'downloading') ??
    live.find((e) => e.status === 'installing') ??
    live[0]
  );
}

/**
 * The 28px status strip's one line (design §3):
 * `<title> · <percent> · <speed>`, or `null` when nothing is live and the
 * strip hides itself. The download arrow that used to prefix this string is
 * an `<Icon name="download">` in `DownloadsFooter.svelte`: `⬇` fell back to
 * a different typeface mid-sentence, and as an emoji-defaulted character it
 * could render as a coloured bitmap inside a `var(--text-h)` status line.
 *
 * An unmeasurable percent renders as an em dash rather than a fake `0%`,
 * and the speed slot carries the phase word when there is no byte rate to
 * show (an install reads local bytes, and a queued job has not started).
 */
export function footerLine(entries: DownloadEntry[]): string | null {
  const current = currentTransfer(entries);
  if (current === null) return null;
  if (current.job === 'backup') return backupFooterLine(current);

  const dash = '—';
  let pct = dash;
  let speed: string;
  switch (current.status) {
    case 'downloading':
      if (current.total_bytes > 0) pct = `${percent(current.downloaded_bytes, current.total_bytes)}%`;
      speed = `${formatSize(current.speed_bps)}/s`;
      break;
    case 'installing':
      if (current.install_total_bytes > 0) {
        pct = `${percent(current.install_processed_bytes, current.install_total_bytes)}%`;
      }
      speed = 'Installing';
      break;
    case 'cancelling':
      speed = 'Cancelling';
      break;
    default:
      speed = 'Queued';
      break;
  }
  // A library move reads and writes local disks: the phase word, never a
  // network rate.
  if (current.kind === 'library_move') speed = 'Moving';
  // An install from a drive names the drive instead of a network rate.
  if (copiesFromDrive(current) && current.status === 'downloading') speed = `Copying from ${current.drive_label}`;
  return `${current.title} · ${pct} · ${speed}`;
}

/** "Backing up Velvet Static to Couch kit · 45% · downloading". */
function backupFooterLine(e: DownloadEntry): string {
  let pct = '—';
  let phase: string;
  switch (e.status) {
    case 'downloading':
      if (e.total_bytes > 0) pct = `${percent(e.downloaded_bytes, e.total_bytes)}%`;
      phase = 'downloading';
      break;
    case 'installing':
      if (e.install_total_bytes > 0) pct = `${percent(e.install_processed_bytes, e.install_total_bytes)}%`;
      phase = 'writing';
      break;
    case 'cancelling':
      phase = 'cancelling';
      break;
    default:
      phase = 'waiting';
      break;
  }
  return `Backing up ${e.title} to ${packName(e)} · ${pct} · ${phase}`;
}

/**
 * Time remaining for a download with a known total and a measured rate
 * (D-UI-6 names an ETA): `<s>s left`, `<m>m <s>s left`, `<h>h <m>m left`,
 * rounded up. Empty for every other state — an install reads local bytes at
 * a rate the backend does not report, and a queued job has no rate yet.
 */
export function etaText(e: DownloadEntry): string {
  if (e.kind === 'library_move') return '';
  if (e.status !== 'downloading' || e.total_bytes <= 0 || e.speed_bps <= 0) return '';
  const remaining = Math.max(0, e.total_bytes - e.downloaded_bytes);
  const secs = Math.ceil(remaining / e.speed_bps);
  if (secs >= 3600) return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m left`;
  if (secs >= 60) return `${Math.floor(secs / 60)}m ${secs % 60}s left`;
  return `${secs}s left`;
}

/**
 * The one-line caption under a row's sparkline panel: the network rate and
 * the ETA while bytes are moving over the network, the phase word while the
 * install writes to disk, blank for queued and terminal rows.
 */
export function graphCaption(e: DownloadEntry): string {
  if (e.kind === 'library_move') {
    if (e.status === 'downloading') return 'Copying files';
    if (e.status === 'installing') return 'Updating paths';
    return '';
  }
  switch (e.status) {
    case 'downloading':
    case 'cancelling': {
      const rate = `${formatSize(e.speed_bps)}/s`;
      const eta = etaText(e);
      return eta === '' ? rate : `${rate} · ${eta}`;
    }
    case 'installing':
      return e.job === 'backup' ? 'Writing to drive' : 'Writing to disk';
    default:
      return '';
  }
}
