// Entering and leaving TV mode on the same view (checklist items 521, 523).
// Pure: which TV tab a desktop view opens, and which desktop view a TV tab
// goes back to. The four views and the four tabs are the same places.
import { SETTINGS_PAGES, type SettingsPage } from './settings/pages';
import type { View } from './shell';
import type { TvTab } from './tv/input/press';

/** Settings › <pane> in TV mode: the desktop's pages plus TV mode, in the
 *  TV rail's order, by name (TVRommStorefront README § Settings: Connection
 *  is gone; Stores is Game stores, after Cloud saves). */
export const TV_PANES = [
  'appearance',
  'backup-drives',
  'cloud-saves',
  'stores',
  'library',
  'retroachievements',
  'systems',
  'tv',
  'updates',
] as const;
export type TvPane = (typeof TV_PANES)[number];

export const TV_PANE_LABELS: Record<TvPane, string> = {
  appearance: 'Appearance',
  'backup-drives': 'Backup drives',
  'cloud-saves': 'Cloud saves',
  library: 'Library',
  retroachievements: 'RetroAchievements',
  stores: 'Game stores',
  systems: 'Systems',
  tv: 'TV mode',
  updates: 'Updates',
};

/** The TV tab a desktop view opens. TV mode has no Server tab: the legacy
 *  Server view opens the Library, where RomM is a group. */
export function tvTabFor(view: View): TvTab {
  return view === 'server' ? 'library' : view;
}

/** The TV pane a desktop Settings page opens on. The legacy Connection page
 *  opens Game stores, where RomM's page is. */
export function tvPaneFor(page: SettingsPage): TvPane {
  const name: string = page;
  return name === 'connection' ? 'stores' : (page as TvPane);
}

/** The desktop view a TV tab leaves to, and the Settings page when it is
 *  Settings (TV mode's own pane has no desktop page: Appearance, the first;
 *  Stores goes to the desktop's Stores page once it has one). */
export function desktopViewFor(tab: TvTab, pane: TvPane): { view: View; settingsPage: SettingsPage | null } {
  if (tab !== 'settings') return { view: tab, settingsPage: null };
  // A pane with no desktop page of its own yet opens Appearance, the first.
  const page = (SETTINGS_PAGES as readonly string[]).includes(pane) ? (pane as SettingsPage) : 'appearance';
  return { view: 'settings', settingsPage: page };
}
