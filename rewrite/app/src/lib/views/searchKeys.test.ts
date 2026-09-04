import { describe, expect, it } from 'vitest';
import { chordBlocked, isSearchChord, shouldFocusSearch, type ChordContext, type ChordEvent } from './searchKeys';

const key = (over: Partial<ChordEvent> = {}): ChordEvent => ({
  key: 'f',
  ctrlKey: true,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...over,
});

const ctx = (over: Partial<ChordContext> = {}): ChordContext => ({
  dialogOpen: false,
  activeTag: null,
  activeEditable: false,
  ...over,
});

describe('isSearchChord', () => {
  it('accepts Ctrl+F and Cmd+F, in either case', () => {
    expect(isSearchChord(key())).toBe(true);
    expect(isSearchChord(key({ ctrlKey: false, metaKey: true }))).toBe(true);
    expect(isSearchChord(key({ key: 'F' }))).toBe(true);
  });

  it('rejects a bare f, another letter, and the Alt/Shift variants', () => {
    expect(isSearchChord(key({ ctrlKey: false }))).toBe(false);
    expect(isSearchChord(key({ key: 'g' }))).toBe(false);
    expect(isSearchChord(key({ altKey: true }))).toBe(false);
    expect(isSearchChord(key({ shiftKey: true }))).toBe(false);
  });
});

describe('chordBlocked', () => {
  it('passes an idle document and a focused button', () => {
    expect(chordBlocked(ctx())).toBe(false);
    expect(chordBlocked(ctx({ activeTag: 'BUTTON' }))).toBe(false);
  });

  it('blocks while a dialog is open', () => {
    expect(chordBlocked(ctx({ dialogOpen: true }))).toBe(true);
  });

  it('blocks every text-entry control and contenteditable', () => {
    for (const tag of ['INPUT', 'TEXTAREA', 'SELECT']) {
      expect(chordBlocked(ctx({ activeTag: tag }))).toBe(true);
    }
    expect(chordBlocked(ctx({ activeTag: 'DIV', activeEditable: true }))).toBe(true);
  });

  it('blocks the grid views\u2019 arrow navigation while a toolbar select has focus', () => {
    // The grid views reuse this guard for Arrow keys, not just Ctrl+F: a
    // focused `library-sort`/`library-size` must keep its own Up/Down, while
    // a focused rail button must not stop the grid moving.
    expect(chordBlocked(ctx({ activeTag: 'SELECT' }))).toBe(true);
    expect(chordBlocked(ctx({ activeTag: 'BUTTON' }))).toBe(false);
  });
});

describe('shouldFocusSearch', () => {
  it('is the chord and the block test together', () => {
    expect(shouldFocusSearch(key(), ctx())).toBe(true);
    expect(shouldFocusSearch(key(), ctx({ dialogOpen: true }))).toBe(false);
    expect(shouldFocusSearch(key(), ctx({ activeTag: 'INPUT' }))).toBe(false);
    expect(shouldFocusSearch(key({ key: 'k' }), ctx())).toBe(false);
  });
});
