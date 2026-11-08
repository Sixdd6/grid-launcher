import { describe, expect, it } from 'vitest';
import {
  BLUR_DEFAULT,
  BLUR_MAX,
  clampBlur,
  artOpacity,
  artVisible,
  clampFade,
  FADE_DEFAULT,
  FADE_MAX,
  normalizeTheme,
  resolveTheme,
  themeAttribute,
  themeFromStorageValue,
  easesThemeSwitch,
  nextTheme,
  THEME_EASE_WINDOW_MS,
  themeToggleIcon,
  themeToggleLabel,
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
    expect(clampFade(100)).toBe(100);
  });
  it('clamps out-of-range values instead of rejecting them', () => {
    expect(clampFade(-5)).toBe(0);
    expect(clampFade(120)).toBe(100);
  });
  it('rounds fractional slider values and falls back on garbage', () => {
    expect(clampFade(30.6)).toBe(31);
    expect(clampFade(Number.NaN)).toBe(FADE_DEFAULT);
    expect(FADE_DEFAULT).toBe(50);
  });
});

// User ruling 2026-11-07: the fade is a real fade. 0 is the art at full
// strength, 100 is the window background alone, and fully faded is off.
describe('artOpacity and artVisible', () => {
  it('maps the fade to the art layer opacity, falling as the fade rises', () => {
    expect(artOpacity(0)).toBe(1);
    expect(artOpacity(50)).toBe(0.5);
    expect(artOpacity(FADE_MAX)).toBe(0);
    expect(artOpacity(150)).toBe(0);
  });
  it('turns the art off only when fully faded', () => {
    expect(artVisible(0)).toBe(true);
    expect(artVisible(99)).toBe(true);
    expect(artVisible(FADE_MAX)).toBe(false);
  });
});

describe('clampBlur', () => {
  it('keeps values inside the design range', () => {
    expect(clampBlur(0)).toBe(0);
    expect(clampBlur(12)).toBe(12);
    expect(clampBlur(BLUR_MAX)).toBe(40);
  });
  it('clamps out-of-range values instead of rejecting them', () => {
    expect(clampBlur(-1)).toBe(0);
    expect(clampBlur(41)).toBe(40);
  });
  it('rounds fractional slider values and falls back on garbage', () => {
    expect(clampBlur(12.6)).toBe(13);
    expect(clampBlur(Number.NaN)).toBe(BLUR_DEFAULT);
    expect(BLUR_DEFAULT).toBe(2);
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

describe('the light / dark toggle', () => {
  it('switches to the other mode', () => {
    expect(nextTheme('light')).toBe('dark');
    expect(nextTheme('dark')).toBe('light');
  });

  it('names the switch it makes', () => {
    expect(themeToggleLabel('light')).toBe('Switch to dark mode');
    expect(themeToggleLabel('dark')).toBe('Switch to light mode');
  });

  it('shows the current mode: sun in light, moon in dark', () => {
    expect(themeToggleIcon('light')).toBe('sun');
    expect(themeToggleIcon('dark')).toBe('moon');
  });

  it('eases the switch unless reduced motion is asked for', () => {
    expect(easesThemeSwitch(false)).toBe(true);
    expect(easesThemeSwitch(true)).toBe(false);
  });

  it('holds the easing class a little past the 240ms transition', () => {
    expect(THEME_EASE_WINDOW_MS).toBeGreaterThan(240);
    expect(THEME_EASE_WINDOW_MS).toBeLessThan(500);
  });
});
