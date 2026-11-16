import { describe, expect, it } from 'vitest';
import { buildHints, hintLeft, NO_CONTROLLER_LINE, type HintInput } from './hints';

const base = (o: Partial<HintInput> = {}): HintInput => ({
  layer: null,
  focus: { a: 'Select', disabled: false, x: null },
  select: null,
  screen: [],
  pages: 0,
  startup: false,
  main: false,
  ...o,
});
const labels = (i: HintInput) => buildHints(i).map((h) => `${h.button} ${h.label}`);

describe('buildHints (items 128–133)', () => {
  it('A, X, the screen hints, then Menu at a tab top level', () => {
    expect(labels(base({ focus: { a: 'Details', disabled: false, x: 'Play' }, screen: [{ button: 'Y', label: 'Search' }] }))).toEqual([
      'A Details',
      'X Play',
      'Y Search',
      'MENU Menu',
    ]);
  });
  it('B Back on pages', () => {
    expect(labels(base({ pages: 1 }))).toEqual(['A Select', 'B Back', 'MENU Menu']);
  });
  it('no A on a disabled item', () => {
    expect(labels(base({ focus: { a: 'Select', disabled: true, x: null } }))).toEqual(['MENU Menu']);
  });
  it('no Back on startup screens, but Menu (user decision 2026-11-10)', () => {
    expect(labels(base({ startup: true, pages: 2 }))).toEqual(['A Select', 'MENU Menu']);
    expect(labels(base({ startup: true, main: true, screen: [{ button: 'MENU', label: 'Skip for now' }] }))).toEqual(['A Select', 'MENU Skip for now']);
  });
  it('no Menu where Start presses the page main button', () => {
    expect(labels(base({ pages: 1, main: true, screen: [{ button: 'MENU', label: 'Backup' }] }))).toEqual(['A Select', 'MENU Backup', 'B Back']);
  });
  it('an overlay: its own hints, else Select and Back (no Back when blocking)', () => {
    expect(labels(base({ layer: { hints: null, blocking: false } }))).toEqual(['A Select', 'B Back']);
    expect(labels(base({ layer: { hints: null, blocking: true } }))).toEqual(['A Select']);
    expect(labels(base({ layer: { hints: [{ button: 'B', label: 'Close' }], blocking: false } }))).toEqual(['B Close']);
  });
  it('select mode replaces the rest after A', () => {
    expect(labels(base({ select: [{ button: 'A', label: 'Pick' }] }))).toEqual(['A Pick']);
  });
});

describe('hintLeft (ruling 24; TVRommStorefront: no offline line)', () => {
  it('the controller in use, or the no-controller line', () => {
    expect(hintLeft({ padName: 'Xbox Wireless Controller' })).toEqual({ kind: 'pad', text: 'Xbox Wireless Controller' });
    expect(hintLeft({ padName: null }).text).toBe(NO_CONTROLLER_LINE);
    expect(NO_CONTROLLER_LINE).toBe('No controller connected');
  });
});
