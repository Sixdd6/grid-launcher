// The top bar's right side (checklist items 124–125, 517; ruling 21). Pure.
import type { DownloadEntry } from '../api';
import { percent } from '../downloads/format';

export type Connection = { dot: 'online' | 'offline' | 'none'; text: string };

/**
 * A green dot and the user when connected; a grey dot and "Offline · no
 * server" when GRID is used without a server; a red dot and "Offline" when
 * the server can't be reached.
 */
export function connection(s: { connected: boolean; offline: boolean; username: string }): Connection {
  if (s.connected) return { dot: 'online', text: s.username };
  if (s.offline) return { dot: 'none', text: 'Offline · no server' };
  return { dot: 'offline', text: 'Offline' };
}

export type ActiveDownload = { id: number; title: string; pct: number };

/**
 * The live download the top bar shows (title, bar, percent): the row
 * downloading, else the one installing. A backup row shows like any other
 * (ruling 21). Queued rows don't show.
 */
export function activeDownload(entries: readonly DownloadEntry[]): ActiveDownload | null {
  const e = entries.find((x) => x.status === 'downloading') ?? entries.find((x) => x.status === 'installing');
  if (!e) return null;
  const pct = e.status === 'installing' && e.install_total_bytes > 0
    ? percent(e.install_processed_bytes, e.install_total_bytes)
    : percent(e.downloaded_bytes, e.total_bytes);
  return { id: e.id, title: e.title, pct: Math.round(pct) };
}

export const TAB_NAMES = { library: 'Library', server: 'Server', downloads: 'Downloads', settings: 'Settings' } as const;
