// The top bar's right side (checklist item 124; ruling 21): the live
// download. No connection dot or user (TVRommStorefront README § The shell).
// Pure.
import type { DownloadEntry } from '../api';
import { percent } from '../downloads/format';

export type ActiveDownload = { id: number; title: string; pct: number };

/**
 * The live download the top bar shows (title, bar, percent): the row
 * downloading, else the one installing. A backup row shows like any other
 * (ruling 21). Queued rows don't show. A Steam install shows only when
 * nothing GRID runs is live.
 */
export function activeDownload(entries: readonly DownloadEntry[]): ActiveDownload | null {
  // A job GRID runs comes before an install Steam runs (the desktop strip's rule).
  const own = entries.filter((x) => x.kind !== 'store_external');
  const pick = (list: readonly DownloadEntry[]) => list.find((x) => x.status === 'downloading') ?? list.find((x) => x.status === 'installing');
  const e = pick(own) ?? pick(entries);
  if (!e) return null;
  const pct = e.status === 'installing' && e.install_total_bytes > 0
    ? percent(e.install_processed_bytes, e.install_total_bytes)
    : percent(e.downloaded_bytes, e.total_bytes);
  return { id: e.id, title: e.title, pct: Math.round(pct) };
}

export const TAB_NAMES = { library: 'Library', downloads: 'Downloads', settings: 'Settings' } as const;
