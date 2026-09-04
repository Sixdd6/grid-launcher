import { describe, expect, it } from 'vitest';
import { LATER_STEP_TEXT, SETTINGS_PAGES, settingsPageLabel, settingsRailEntries } from './pages';

describe('settings rail', () => {
  it('lists the five pages of design §10, in order', () => {
    expect([...SETTINGS_PAGES]).toEqual([
      'connection',
      'cloud-saves',
      'retroachievements',
      'updates',
      'appearance',
    ]);
  });

  it('labels every page', () => {
    expect(settingsPageLabel('connection')).toBe('Connection');
    expect(settingsPageLabel('cloud-saves')).toBe('Cloud saves');
    expect(settingsPageLabel('retroachievements')).toBe('RetroAchievements');
    expect(settingsPageLabel('updates')).toBe('Updates');
    expect(settingsPageLabel('appearance')).toBe('Appearance');
  });

  it('holds the placeholder copy verbatim', () => {
    expect(LATER_STEP_TEXT).toBe('Coming in a later step');
  });
});

describe('settingsRailEntries', () => {
  it('builds one entry per page with the §11 ids, the heading on the first', () => {
    const entries = settingsRailEntries('updates');
    expect(entries.map((e) => e.testId)).toEqual([
      'settings-nav-connection',
      'settings-nav-cloud-saves',
      'settings-nav-retroachievements',
      'settings-nav-updates',
      'settings-nav-appearance',
    ]);
    expect(entries.map((e) => e.selected)).toEqual([false, false, false, true, false]);
    expect(entries[0].heading).toBe('SETTINGS');
    expect(entries.slice(1).every((e) => e.heading === undefined)).toBe(true);
  });
});
