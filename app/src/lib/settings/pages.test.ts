import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SETTINGS_PAGE,
  SETTINGS_PAGES,
  settingsPageLabel,
  settingsRailEntries,
} from './pages';

describe('settings rail', () => {
  it('lists the pages alphabetically by label', () => {
    expect([...SETTINGS_PAGES]).toEqual([
      'appearance',
      'cloud-saves',
      'connection',
      'library',
      'retroachievements',
      'systems',
      'updates',
    ]);
    const labels = SETTINGS_PAGES.map(settingsPageLabel);
    expect(labels).toEqual([...labels].sort((a, b) => a.localeCompare(b)));
  });

  it('opens on Appearance, the first page', () => {
    expect(DEFAULT_SETTINGS_PAGE).toBe('appearance');
    expect(SETTINGS_PAGES[0]).toBe(DEFAULT_SETTINGS_PAGE);
  });

  it('labels every page', () => {
    expect(settingsPageLabel('connection')).toBe('Connection');
    expect(settingsPageLabel('library')).toBe('Library');
    expect(settingsPageLabel('cloud-saves')).toBe('Cloud saves');
    expect(settingsPageLabel('retroachievements')).toBe('RetroAchievements');
    expect(settingsPageLabel('systems')).toBe('Systems');
    expect(settingsPageLabel('updates')).toBe('Updates');
    expect(settingsPageLabel('appearance')).toBe('Appearance');
  });
});

describe('settingsRailEntries', () => {
  it('builds one entry per page with the §11 ids, the heading on the first', () => {
    const entries = settingsRailEntries('updates');
    expect(entries.map((e) => e.testId)).toEqual([
      'settings-nav-appearance',
      'settings-nav-cloud-saves',
      'settings-nav-connection',
      'settings-nav-library',
      'settings-nav-retroachievements',
      'settings-nav-systems',
      'settings-nav-updates',
    ]);
    expect(entries.map((e) => e.selected)).toEqual([false, false, false, false, false, false, true]);
    expect(entries[0].heading).toBe('SETTINGS');
    expect(entries.slice(1).every((e) => e.heading === undefined)).toBe(true);
  });
});
