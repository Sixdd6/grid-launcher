// The Settings rail (design §10): five pages, their labels, and the rail
// entries Settings.svelte hands to RailPane. Pure.

export const SETTINGS_PAGES = [
  'connection',
  'cloud-saves',
  'retroachievements',
  'updates',
  'appearance',
] as const;

export type SettingsPage = (typeof SETTINGS_PAGES)[number];

const LABELS: Record<SettingsPage, string> = {
  connection: 'Connection',
  'cloud-saves': 'Cloud saves',
  retroachievements: 'RetroAchievements',
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
