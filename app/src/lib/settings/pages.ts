// The Settings rail (design §10): seven pages, their labels, and the rail
// entries Settings.svelte hands to RailPane. Pure.

/** Alphabetical by label (ProposedApp README, "Settings rail"). */
export const SETTINGS_PAGES = [
  'appearance',
  'cloud-saves',
  'connection',
  'library',
  'retroachievements',
  'systems',
  'updates',
] as const;

export type SettingsPage = (typeof SETTINGS_PAGES)[number];

/** The page Settings opens on: the first in the rail. */
export const DEFAULT_SETTINGS_PAGE: SettingsPage = SETTINGS_PAGES[0];

const LABELS: Record<SettingsPage, string> = {
  connection: 'Connection',
  library: 'Library',
  'cloud-saves': 'Cloud saves',
  retroachievements: 'RetroAchievements',
  systems: 'Systems',
  updates: 'Updates',
  appearance: 'Appearance',
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
