import { beforeEach, describe, expect, it } from 'vitest';
import {
  DETAILS_TABS,
  DETAILS_TAB_LABELS,
  isDetailsTab,
  rememberTab,
  rememberedTab,
  resetRememberedTab,
  tabTestId,
} from './tabs';

beforeEach(() => resetRememberedTab());

describe('the tab set', () => {
  it('is exactly design §7 four tabs, in order', () => {
    expect(DETAILS_TABS).toEqual(['overview', 'media', 'saves', 'files']);
  });

  it('labels every tab', () => {
    expect(DETAILS_TABS.map((t) => DETAILS_TAB_LABELS[t])).toEqual([
      'Overview',
      'Media',
      'Saves',
      'Files',
    ]);
  });

  it('builds the design §11 test id', () => {
    expect(tabTestId('media')).toBe('details-tab-media');
  });

  it('recognizes only the four names', () => {
    expect(isDetailsTab('files')).toBe(true);
    expect(isDetailsTab('metadata')).toBe(false);
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
