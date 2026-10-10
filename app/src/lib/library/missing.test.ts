import { describe, expect, it } from 'vitest';
import {
  cardPrimaryLabel,
  isMissing,
  MISSING_BADGE_TEXT,
  MISSING_REASON,
  missingActions,
} from './missing';

describe('isMissing (9c)', () => {
  it('is true only for a row the backend flagged', () => {
    expect(isMissing({ missing: true })).toBe(true);
    expect(isMissing({ missing: false })).toBe(false);
    // Rows from commands other than list_installed carry no flag.
    expect(isMissing({})).toBe(false);
    expect(isMissing(null)).toBe(false);
    expect(isMissing(undefined)).toBe(false);
  });
});

describe('cardPrimaryLabel (9c)', () => {
  it('offers Locate… instead of Play for a missing game', () => {
    expect(cardPrimaryLabel(true, false)).toBe('Play');
    expect(cardPrimaryLabel(true, true)).toBe('Locate…');
    expect(cardPrimaryLabel(false, false)).toBe('Install');
  });

  it('has a badge text', () => {
    expect(MISSING_BADGE_TEXT).toBe('MISSING');
  });
});

describe('missingActions (9c)', () => {
  it('shows the missing block and blocks Play and Uninstall with the reason', () => {
    expect(missingActions({ missing: true })).toEqual({
      showMissing: true,
      playBlocked: MISSING_REASON,
      uninstallBlocked: MISSING_REASON,
    });
  });

  it('shows nothing and blocks nothing for a present game or no row', () => {
    const none = { showMissing: false, playBlocked: '', uninstallBlocked: '' };
    expect(missingActions({ missing: false })).toEqual(none);
    expect(missingActions(null)).toEqual(none);
  });
});
