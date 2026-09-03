// Pure display formatting for the downloads/installs UI. Ported verbatim from
// grid_launcher/library/downloads.py (see docs/porting/03-library-install.md,
// "Display text per status" and "Aggregate status text"). No store imports here
// so this module stays import-cycle-free.
import type { DownloadEntry, DownloadKind, DownloadStatus } from '../api';

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
  switch (e.status) {
    case 'queued':
      return 'Queued';
    case 'downloading':
      if (e.total_bytes > 0) {
        return (
          `Downloading ${percent(e.downloaded_bytes, e.total_bytes)}% • ` +
          `${formatSize(e.downloaded_bytes)} / ${formatSize(e.total_bytes)} • ${formatSize(e.speed_bps)}/s`
        );
      }
      return `Downloading • ${formatSize(e.downloaded_bytes)} • ${formatSize(e.speed_bps)}/s`;
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

const LIVE_STATUSES: DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

export function aggregate(entries: DownloadEntry[]): string {
  const hasLive = entries.some((e) => LIVE_STATUSES.includes(e.status));
  if (!hasLive) return '';

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
    default:
      return '';
  }
}

export type DownloadAction = 'cancel' | 'installing' | 'retry-dismiss' | 'dismiss' | 'none';

/**
 * Which buttons a drawer row offers.
 *
 * A `firmware` row is the background firmware installer's external entry: it
 * owns no queue slot, so `cancel_install` cannot stop it and `retry_install`
 * returns without doing anything (`JobKey::External`). It therefore gets no
 * button at all while it is live, and only `Dismiss` once it is terminal —
 * never a Cancel or Retry that would do nothing. Every other kind keeps the
 * behavior it had.
 */
export function actionFor(status: DownloadStatus, kind: DownloadKind = 'base'): DownloadAction {
  if (kind === 'firmware') {
    return LIVE_STATUSES.includes(status) ? 'none' : 'dismiss';
  }
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
