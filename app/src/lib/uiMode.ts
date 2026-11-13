// Entering and leaving TV mode on the same view (checklist items 521, 523).
// Pure: which TV tab a desktop view opens, and which desktop view a TV tab
// goes back to. The four views and the four tabs are the same places.
import type { SettingsPage } from './settings/pages';
import type { View } from './shell';
import type { TvTab } from './tv/input/press';

/** Settings › <pane> in TV mode: the desktop's pages plus TV mode, in the
 *  TV rail's order (TVMode README § Tabs › Settings). */
export const TV_PANES = [
  'appearance',
  'backup-drives',
  'cloud-saves',
  'connection',
  'library',
  'retroachievements',
  // Settings › Stores (GameStores / TVStores); TV mode's pane is package F's.
  'stores',
  'systems',
  'tv',
  'updates',
] as const;
export type TvPane = (typeof TV_PANES)[number];

export const TV_PANE_LABELS: Record<TvPane, string> = {
  appearance: 'Appearance',
  'backup-drives': 'Backup drives',
  'cloud-saves': 'Cloud saves',
  connection: 'Connection',
  library: 'Library',
  retroachievements: 'RetroAchievements',
  stores: 'Stores',
  systems: 'Systems',
  tv: 'TV mode',
  updates: 'Updates',
};

export function tvTabFor(view: View): TvTab {
  return view;
}

/** The TV pane a desktop Settings page opens on. */
export function tvPaneFor(page: SettingsPage): TvPane {
  return page;
}

/** The desktop view a TV tab leaves to, and the Settings page when it is
 *  Settings (TV mode's own pane has no desktop page: Appearance, the first). */
export function desktopViewFor(tab: TvTab, pane: TvPane): { view: View; settingsPage: SettingsPage | null } {
  if (tab !== 'settings') return { view: tab, settingsPage: null };
  return { view: 'settings', settingsPage: pane === 'tv' ? 'appearance' : pane };
}
