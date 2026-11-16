import { describe, expect, it } from 'vitest';
import { hostOf, initialPlace, STOREFRONT_VIEWS, storefrontViewForDigit, viewLabel } from './shell';

describe('viewLabel / hostOf', () => {
  it('labels every pill', () => {
    expect(viewLabel('library')).toBe('Library');
    expect(viewLabel('downloads')).toBe('Downloads');
    expect(viewLabel('settings')).toBe('Settings');
  });
  it('hostOf takes the host and port', () => {
    expect(hostOf('https://romm.example:8080/base')).toBe('romm.example:8080');
  });
  it('hostOf falls back to the raw string', () => {
    expect(hostOf('not a url')).toBe('not a url');
  });
});

describe('initialPlace (RomM as a storefront, plan 0.5, decision 16)', () => {
  it.each([
    // [rommConnected, openOn, rail]
    [true, 'automatic', 'romm:discover'],
    [false, 'automatic', 'all'],
    [true, 'discover', 'romm:discover'],
    [false, 'discover', 'all'],
    [true, 'library', 'all'],
    [false, 'library', 'all'],
  ] as const)('connected=%s, %s → %s', (connected, openOn, rail) => {
    expect(initialPlace(connected, openOn)).toEqual({ view: 'library', rail, drive: null });
  });
  it('defaults to automatic', () => {
    expect(initialPlace(true).rail).toBe('romm:discover');
  });
});

describe('storefront views', () => {
  it('are Library, Downloads, Settings on Ctrl+1..3', () => {
    expect(STOREFRONT_VIEWS).toEqual(['library', 'downloads', 'settings']);
    expect(storefrontViewForDigit('1')).toBe('library');
    expect(storefrontViewForDigit('2')).toBe('downloads');
    expect(storefrontViewForDigit('3')).toBe('settings');
    expect(storefrontViewForDigit('4')).toBeNull();
  });
  it('ignore every other key, including 0 and non-digits', () => {
    expect(storefrontViewForDigit('0')).toBeNull();
    expect(storefrontViewForDigit('f')).toBeNull();
    expect(storefrontViewForDigit('')).toBeNull();
    expect(storefrontViewForDigit('11')).toBeNull();
  });
});
