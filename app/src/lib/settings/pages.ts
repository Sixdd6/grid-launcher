// The Settings rail (design §10): eight pages (Game stores from GameStores
// and RommStorefront; Connection is gone, its job is the RomM card's), their
// labels, and the rail entries Settings.svelte hands to RailPane. Pure.

/** Alphabetical by label (RommStorefront README, "Settings": Appearance,
 *  Backup drives, Cloud saves, Game stores, Library, RetroAchievements,
 *  Systems, Updates). The key `stores` is kept from the Stores page. */
export const SETTINGS_PAGES = [
  'appearance',
  'backup-drives',
  'cloud-saves',
  'stores',
  'library',
  'retroachievements',
  'systems',
  'updates',
] as const;

export type SettingsPage = (typeof SETTINGS_PAGES)[number];

/** The page Settings opens on: the first in the rail. */
export const DEFAULT_SETTINGS_PAGE: SettingsPage = SETTINGS_PAGES[0];

const LABELS: Record<SettingsPage, string> = {
  library: 'Library',
  'cloud-saves': 'Cloud saves',
  retroachievements: 'RetroAchievements',
  stores: 'Game stores',
  systems: 'Systems',
  updates: 'Updates',
  appearance: 'Appearance',
  'backup-drives': 'Backup drives',
};

export function settingsPageLabel(page: SettingsPage): string {
  return LABELS[page];
}

/** One rail row, the shape `RailPane.svelte` renders (no count: Settings has none). */
export type SettingsRailEntry = {
  key: SettingsPage;
  testId: string;
  label: string;
  selected: boolean;
  heading?: string;
};

export function settingsRailEntries(selected: SettingsPage): SettingsRailEntry[] {
  return SETTINGS_PAGES.map((page, i) => ({
    key: page,
    testId: `settings-nav-${page}`,
    label: LABELS[page],
    selected: page === selected,
    ...(i === 0 ? { heading: 'SETTINGS' } : {}),
  }));
}
