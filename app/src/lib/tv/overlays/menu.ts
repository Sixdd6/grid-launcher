// The Menu panel's words (checklist items 363–364; tvmode.js `guideMenu`;
// TVRommStorefront README § The shell). Pure: TvMenu draws them and runs
// each item. No Reconnect and no "<user> · <server>" line: RomM's state and
// Reconnect live on its Library group and its Game stores page.
import type { DownloadEntry } from '../../api';

export type MenuItemKey = 'discover' | 'downloads' | 'notifications' | 'settings' | 'controls' | 'desktop' | 'quit';

/** Rows counted as active on Downloads: the ones running and the queued. */
export function activeCount(entries: readonly DownloadEntry[]): number {
  return entries.filter((e) => e.status === 'queued' || e.status === 'downloading' || e.status === 'installing').length;
}

/**
 * The items, in order (item 364). Discover only while RomM is connected or
 * the startup probe runs (`rommLive`): it opens Library › RomM › Discover.
 */
export function menuItems(active: number, notices: number, discover = true): { key: MenuItemKey; label: string }[] {
  return [
    ...(discover ? [{ key: 'discover' as const, label: 'Discover' }] : []),
    { key: 'downloads', label: active > 0 ? `Downloads · ${active} active` : 'Downloads' },
    { key: 'notifications', label: `Notifications · ${notices}` },
    { key: 'settings', label: 'Settings' },
    // Shows the welcome card again (user decision 2026-11-10).
    { key: 'controls', label: 'Controls' },
    { key: 'desktop', label: 'Switch to desktop mode' },
    { key: 'quit', label: 'Quit GRID' },
  ];
}

/**
 * The Menu on a startup screen (the welcome, Choose your systems, the
 * unreachable library): only the ways out, so a controller alone can leave
 * TV mode before GRID is set up (user decision 2026-11-10).
 */
export function startupMenuItems(): { key: MenuItemKey; label: string }[] {
  return [
    { key: 'desktop', label: 'Switch to desktop mode' },
    { key: 'quit', label: 'Quit GRID' },
  ];
}
