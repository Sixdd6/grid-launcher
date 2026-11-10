import { describe, expect, it } from 'vitest';
import { tvTheme } from './theme';
import { clockText } from './clock.svelte';

describe('tvTheme (ruling 22)', () => {
  it('shows Follow system as dark; Dark and Light as chosen', () => {
    expect(tvTheme('system')).toBe('dark');
    expect(tvTheme('dark')).toBe('dark');
    expect(tvTheme('light')).toBe('light');
  });
});

describe('clockText (item 126)', () => {
  it('is HH:MM', () => {
    expect(clockText(new Date(2026, 10, 10, 9, 5))).toBe('09:05');
    expect(clockText(new Date(2026, 10, 10, 21, 40))).toBe('21:40');
  });
});
