import { describe, expect, it } from 'vitest';
import { LATER_STEP_TEXT, SETTINGS_PAGES, settingsPageLabel } from './settings';

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
