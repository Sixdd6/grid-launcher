import { describe, expect, it } from 'vitest';
import { desktopViewFor, TV_PANE_LABELS, TV_PANES, tvPaneFor, tvTabFor } from './uiMode';
import { SETTINGS_PAGES } from './settings/pages';

describe('uiMode mapping (items 521, 523)', () => {
  it('each desktop view opens its TV tab', () => {
    for (const v of ['library', 'downloads', 'settings'] as const) expect(tvTabFor(v)).toBe(v);
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
    // TV mode's own pane.
    const extra = TV_PANES.filter((p) => !(SETTINGS_PAGES as readonly string[]).includes(p));
    expect(extra).toEqual(['tv']);
  });
  it('the rail is by name: Game stores after Cloud saves, no Connection', () => {
    expect(TV_PANE_LABELS.stores).toBe('Game stores');
    expect(TV_PANES.indexOf('stores')).toBe(TV_PANES.indexOf('cloud-saves') + 1);
    expect(TV_PANES.indexOf('library')).toBe(TV_PANES.indexOf('stores') + 1);
    expect((TV_PANES as readonly string[]).includes('connection')).toBe(false);
  });
});
