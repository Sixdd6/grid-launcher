import { describe, expect, it } from 'vitest';
import { desktopViewFor, TV_PANES, tvPaneFor, tvTabFor } from './uiMode';
import { SETTINGS_PAGES } from './settings/pages';

describe('uiMode mapping (items 521, 523)', () => {
  it('each desktop view opens its TV tab', () => {
    for (const v of ['library', 'server', 'downloads', 'settings'] as const) expect(tvTabFor(v)).toBe(v);
  });
  it('each TV tab leaves to the same desktop view', () => {
    expect(desktopViewFor('library', 'appearance')).toEqual({ view: 'library', settingsPage: null });
    expect(desktopViewFor('downloads', 'tv')).toEqual({ view: 'downloads', settingsPage: null });
  });
  it('Settings keeps its page; TV mode pane goes to Appearance', () => {
    expect(desktopViewFor('settings', 'systems')).toEqual({ view: 'settings', settingsPage: 'systems' });
    expect(desktopViewFor('settings', 'tv')).toEqual({ view: 'settings', settingsPage: 'appearance' });
  });
  it('every desktop Settings page is a TV pane, plus TV mode', () => {
    for (const p of SETTINGS_PAGES) expect(TV_PANES).toContain(tvPaneFor(p));
    expect(TV_PANES).toContain('tv');
    // TV mode's own pane, and Stores until the desktop has its page.
    const extra = TV_PANES.filter((p) => !(SETTINGS_PAGES as readonly string[]).includes(p));
    expect(extra.filter((p) => p !== 'stores')).toEqual(['tv']);
  });
  it('Stores sits between RetroAchievements and Systems', () => {
    expect(TV_PANES.indexOf('stores')).toBe(TV_PANES.indexOf('retroachievements') + 1);
    expect(TV_PANES.indexOf('systems')).toBe(TV_PANES.indexOf('stores') + 1);
  });
});
