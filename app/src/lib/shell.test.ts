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

describe('initialPlace (plan 0.5, decision 16; Discover removed 2026-11-17)', () => {
  it('always opens on Library › All games', () => {
    expect(initialPlace()).toEqual({ view: 'library', rail: 'all', drive: null });
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
