// The Menu panel's words (checklist items 363–364; tvmode.js `guideMenu`).
// Pure: TvMenu draws them and runs each item.
import type { DownloadEntry } from '../../api';
import { hostOf } from '../../shell';

export type MenuItemKey = 'discover' | 'downloads' | 'notifications' | 'settings' | 'reconnect' | 'desktop' | 'quit';

/** The session state the Menu reads. */
export type MenuSession = { connected: boolean; offline: boolean; username: string; serverUrl: string };

/** Used without a server (not merely unable to reach one). */
function noServer(s: Pick<MenuSession, 'connected' | 'offline'>): boolean {
  return s.offline && !s.connected;
}

/**
 * The head's line: "<user> · <server host>", "Offline", or "Offline · no
 * server" when used without a server, like the top bar (item 363; README §
 * Consultant decisions).
 */
export function menuHead(s: MenuSession): string {
  if (s.connected) return `${s.username} · ${hostOf(s.serverUrl)}`;
  return noServer(s) ? 'Offline · no server' : 'Offline';
}

/** Rows counted as active on Downloads: the ones running and the queued. */
export function activeCount(entries: readonly DownloadEntry[]): number {
  return entries.filter((e) => e.status === 'queued' || e.status === 'downloading' || e.status === 'installing').length;
}

/**
 * The items, in order (item 364). Used without a server the Reconnect row
 * reads "Connect to a server…" (same row, which opens Connect).
 */
export function menuItems(active: number, notices: number, s: Pick<MenuSession, 'connected' | 'offline'> = { connected: true, offline: false }): { key: MenuItemKey; label: string }[] {
  return [
    { key: 'discover', label: 'Discover' },
    { key: 'downloads', label: active > 0 ? `Downloads · ${active} active` : 'Downloads' },
    { key: 'notifications', label: `Notifications · ${notices}` },
    { key: 'settings', label: 'Settings' },
    { key: 'reconnect', label: noServer(s) ? 'Connect to a server…' : 'Reconnect' },
    { key: 'desktop', label: 'Switch to desktop mode' },
    { key: 'quit', label: 'Quit GRID' },
  ];
}
