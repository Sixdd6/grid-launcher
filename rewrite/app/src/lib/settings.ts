// The Settings rail (design §10). Only Appearance is built in this plan;
// plan 5 fills in the other four pages.

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

/** The exact line an unbuilt page shows. Asserted by settings.test.ts so it
 *  cannot drift while five call sites reference it. */
export const LATER_STEP_TEXT = 'Coming in a later step';
