import { beforeEach, describe, expect, it } from 'vitest';
import {
  DETAILS_TABS,
  DETAILS_TAB_LABELS,
  isDetailsTab,
  rememberTab,
  rememberedTab,
  resetRememberedTab,
  shownTab,
  tabTestId,
  visibleTabs,
} from './tabs';

beforeEach(() => resetRememberedTab());

describe('the tab set', () => {
  it('is the four design §7 tabs plus Achievements, in order', () => {
    expect(DETAILS_TABS).toEqual(['overview', 'media', 'saves', 'files', 'achievements']);
  });

  it('labels every tab', () => {
    expect(DETAILS_TABS.map((t) => DETAILS_TAB_LABELS[t])).toEqual([
      'Overview',
      'Media',
      'Saves',
      'Files',
      'Achievements',
    ]);
  });

  it('builds the design §11 test id', () => {
    expect(tabTestId('media')).toBe('details-tab-media');
    expect(tabTestId('achievements')).toBe('details-tab-achievements');
  });

  it('recognizes only the five names', () => {
    expect(isDetailsTab('files')).toBe(true);
    expect(isDetailsTab('achievements')).toBe(true);
    expect(isDetailsTab('metadata')).toBe(false);
  });
});

describe('tab visibility', () => {
  it('shows Achievements only when the ROM lists at least one', () => {
    expect(visibleTabs(3)).toEqual(['overview', 'media', 'saves', 'files', 'achievements']);
    expect(visibleTabs(0)).toEqual(['overview', 'media', 'saves', 'files']);
  });

  it('hides Achievements while the detail has not loaded', () => {
    expect(visibleTabs(null)).toEqual(['overview', 'media', 'saves', 'files']);
    expect(visibleTabs(undefined)).toEqual(['overview', 'media', 'saves', 'files']);
  });

  it('falls back to Overview when the chosen tab is hidden for this game', () => {
    expect(shownTab('achievements', visibleTabs(0))).toBe('overview');
    expect(shownTab('achievements', visibleTabs(5))).toBe('achievements');
    expect(shownTab('files', visibleTabs(0))).toBe('files');
  });
});

describe('the remembered tab', () => {
  it('starts on Overview', () => {
    expect(rememberedTab()).toBe('overview');
  });

  it('remembers the last tab across popup opens within the session', () => {
    rememberTab('saves');
    expect(rememberedTab()).toBe('saves');
  });

  it('is module scoped, so a later read sees the last write', () => {
    rememberTab('files');
    rememberTab('media');
    expect(rememberedTab()).toBe('media');
  });
});
