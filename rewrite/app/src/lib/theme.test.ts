import { describe, expect, it } from 'vitest';
import {
  clampFade,
  FADE_DEFAULT,
  normalizeTheme,
  resolveTheme,
  themeAttribute,
  themeFromStorageValue,
} from './theme';

describe('normalizeTheme', () => {
  it('accepts the three stored spellings', () => {
    expect(normalizeTheme('system')).toBe('system');
    expect(normalizeTheme('dark')).toBe('dark');
    expect(normalizeTheme('light')).toBe('light');
  });
  it('falls back to system for anything else, including case and padding', () => {
    expect(normalizeTheme('')).toBe('system');
    expect(normalizeTheme('Dark')).toBe('system');
    expect(normalizeTheme('solarized')).toBe('system');
  });
  it('trims, because a hand-edited config.toml can carry spaces', () => {
    expect(normalizeTheme('  light  ')).toBe('light');
  });
});

describe('resolveTheme', () => {
  it('follows the OS only for the system choice', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
  });
  it('ignores the OS when the user picked a theme', () => {
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('light', true)).toBe('light');
  });
});

describe('themeAttribute', () => {
  it('writes no attribute for system, so the media query decides', () => {
    expect(themeAttribute('system')).toBeNull();
  });
  it('writes the override for an explicit choice', () => {
    expect(themeAttribute('dark')).toBe('dark');
    expect(themeAttribute('light')).toBe('light');
  });
});

describe('clampFade', () => {
  it('keeps values inside the design range', () => {
    expect(clampFade(0)).toBe(0);
    expect(clampFade(25)).toBe(25);
    expect(clampFade(60)).toBe(60);
  });
  it('clamps out-of-range values instead of rejecting them', () => {
    expect(clampFade(-5)).toBe(0);
    expect(clampFade(120)).toBe(60);
  });
  it('rounds fractional slider values and falls back on garbage', () => {
    expect(clampFade(30.6)).toBe(31);
    expect(clampFade(Number.NaN)).toBe(FADE_DEFAULT);
  });
});

describe('themeFromStorageValue', () => {
  it('maps the two explicit spellings straight through', () => {
    expect(themeFromStorageValue('dark')).toBe('dark');
    expect(themeFromStorageValue('light')).toBe('light');
  });
  it('returns null for missing, garbage, or "system" — nothing to pre-apply', () => {
    expect(themeFromStorageValue(null)).toBeNull();
    expect(themeFromStorageValue('')).toBeNull();
    expect(themeFromStorageValue('system')).toBeNull();
    expect(themeFromStorageValue('Dark')).toBeNull();
  });
});
