import { describe, expect, it } from 'vitest';
import { BACK, CARET_LEFT, CARET_RIGHT, DONE, KEY_ROWS, keyFace, OSK_BINDS, oskOpen, oskRepeats, oskShown, oskType, physicalKey, SHIFT, SPACE, SYMBOLS, SYMBOLS_KEY, type Osk } from './osk';

const typeAll = (k: Osk, keys: string[]) => keys.reduce(oskType, k);

describe('layout (items 378, 382)', () => {
  it('has the four rows and the symbols', () => {
    expect(KEY_ROWS.map((r) => r.join(''))).toEqual(['1234567890', 'qwertyuiop', 'asdfghjkl-', 'zxcvbnm./:']);
    expect(SYMBOLS.join(' ')).toBe('@ _ # % ? = & " ! +');
  });
});

describe('typing', () => {
  it('inserts at the caret and deletes before it', () => {
    let k = typeAll(oskOpen('', false), ['a', 'b', 'c']);
    expect(k).toMatchObject({ value: 'abc', caret: 3 });
    k = typeAll(k, [CARET_LEFT, CARET_LEFT, 'x']);
    expect(k).toMatchObject({ value: 'axbc', caret: 2 });
    k = oskType(k, BACK);
    expect(k).toMatchObject({ value: 'abc', caret: 1 });
    k = typeAll(k, [BACK, BACK]);
    expect(k).toMatchObject({ value: 'bc', caret: 0 });
  });

  it('keeps the caret inside the value (LB / RB)', () => {
    let k = oskOpen('ab', false);
    expect(k.caret).toBe(2);
    k = oskType(k, CARET_RIGHT);
    expect(k.caret).toBe(2);
    k = typeAll(k, [CARET_LEFT, CARET_LEFT, CARET_LEFT]);
    expect(k.caret).toBe(0);
  });

  it('Shift capitalises the next letter only (item 381)', () => {
    let k = oskType(oskOpen('', false), SHIFT);
    expect(k.shift).toBe(true);
    expect(keyFace('q', k.shift)).toBe('Q');
    k = oskType(k, keyFace('q', k.shift));
    k = oskType(k, keyFace('w', k.shift));
    expect(k.value).toBe('Qw');
    expect(k.shift).toBe(false);
    // Shift twice turns it off again.
    expect(typeAll(oskOpen('', false), [SHIFT, SHIFT]).shift).toBe(false);
  });

  it('Space adds a space; Done and the symbols key change nothing here', () => {
    const k = oskOpen('a', false);
    expect(oskType(k, SPACE).value).toBe('a ');
    expect(oskType(k, DONE)).toBe(k);
    expect(oskType(k, SYMBOLS_KEY)).toBe(k);
  });
});

describe('masked fields (item 386)', () => {
  it('open empty and show dots', () => {
    const k = typeAll(oskOpen('secret-token', true), ['a', 'b']);
    expect(k.value).toBe('ab');
    expect(oskShown(k, true)).toEqual({ before: '••', after: '' });
    expect(oskShown(oskOpen('hello', false), false)).toEqual({ before: 'hello', after: '' });
  });
});

describe('repeat (items 42–43)', () => {
  it('X, Y, LB and RB repeat; A only on a typing key', () => {
    for (const b of ['X', 'Y', 'LB', 'RB'] as const) expect(oskRepeats(b, null)).toBe(true);
    for (const key of ['q', 'Q', '1', '.', SPACE, BACK]) expect(oskRepeats('A', key)).toBe(true);
    for (const key of [SHIFT, SYMBOLS_KEY, DONE]) expect(oskRepeats('A', key)).toBe(false);
    expect(oskRepeats('A', null)).toBe(false);
    expect(oskRepeats('LT', null)).toBe(false);
    expect(oskRepeats('MENU', null)).toBe(false);
  });
});

describe('shortcuts (item 383)', () => {
  it('X deletes, Y spaces, LB / RB move, LT shifts, Menu is Done', () => {
    expect(OSK_BINDS).toEqual({ X: BACK, Y: SPACE, LB: CARET_LEFT, RB: CARET_RIGHT, LT: SHIFT, MENU: DONE });
  });
});

describe('a physical keyboard (item 74)', () => {
  it('types printable keys, Space included, and Backspace deletes', () => {
    const k = oskOpen('', false);
    expect(physicalKey(k, { key: 'x' })).toEqual({ type: 'x' });
    expect(physicalKey(k, { key: ' ' })).toEqual({ type: ' ' });
    expect(physicalKey(k, { key: 'Backspace' })).toEqual({ type: BACK });
  });
  it('makes Enter Done only after physical typing', () => {
    expect(physicalKey(oskOpen('', false), { key: 'Enter' })).toBeNull();
    expect(physicalKey({ ...oskOpen('', false), phys: true }, { key: 'Enter' })).toEqual({ type: DONE });
  });
  it('leaves arrows, Escape and shortcuts to the controller stand-in', () => {
    const k = oskOpen('', false);
    expect(physicalKey(k, { key: 'ArrowLeft' })).toBeNull();
    expect(physicalKey(k, { key: 'Escape' })).toBeNull();
    expect(physicalKey(k, { key: 'v', ctrl: true })).toBeNull();
  });
});
