// The Menu panel's words (checklist items 363–364; tvmode.js `guideMenu`).
// Pure: TvMenu draws them and runs each item.
import type { DownloadEntry } from '../../api';
import { hostOf } from '../../shell';

export type MenuItemKey = 'discover' | 'downloads' | 'notifications' | 'settings' | 'reconnect' | 'desktop' | 'quit';

/** The head's line: "<user> · <server host>", or "Offline" (item 363). */
export function menuHead(s: { connected: boolean; username: string; serverUrl: string }): string {
  return s.connected ? `${s.username} · ${hostOf(s.serverUrl)}` : 'Offline';
}

/** Rows counted as active on Downloads: the ones running and the queued. */
export function activeCount(entries: readonly DownloadEntry[]): number {
  return entries.filter((e) => e.status === 'queued' || e.status === 'downloading' || e.status === 'installing').length;
}

/** The items, in order (item 364). */
export function menuItems(active: number, notices: number): { key: MenuItemKey; label: string }[] {
  return [
    { key: 'discover', label: 'Discover' },
    { key: 'downloads', label: active > 0 ? `Downloads · ${active} active` : 'Downloads' },
    { key: 'notifications', label: `Notifications · ${notices}` },
    { key: 'settings', label: 'Settings' },
    { key: 'reconnect', label: 'Reconnect' },
    { key: 'desktop', label: 'Switch to desktop mode' },
    { key: 'quit', label: 'Quit GRID' },
  ];
}
