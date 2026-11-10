import { describe, expect, it } from 'vitest';
import {
  normalizeOpenOn,
  normalizeTvAssignments,
  normalizeTvPrompts,
  OPEN_ON_CHOICES,
  OPEN_ON_HINT,
  openOnLabel,
} from './openOn';

describe('Open GRID on', () => {
  it('offers Automatic, Server › Discover and Library, in that order', () => {
    expect(OPEN_ON_CHOICES.map(openOnLabel)).toEqual([
      'Automatic: Discover when connected, else Library',
      'Server › Discover',
      'Library',
    ]);
    expect(OPEN_ON_HINT).toBe('Here and in TV mode');
  });

  it('normalises like the Rust side', () => {
    expect(normalizeOpenOn('discover')).toBe('discover');
    expect(normalizeOpenOn(' library ')).toBe('library');
    expect(normalizeOpenOn('Library')).toBe('automatic');
    expect(normalizeOpenOn('home')).toBe('automatic');
    expect(normalizeOpenOn('')).toBe('automatic');
  });
});

describe('TV keys', () => {
  it('keeps the known prompt sets and falls back to match', () => {
    for (const v of ['match', 'xbox', 'ps', 'nintendo', '8bitdo'] as const) expect(normalizeTvPrompts(v)).toBe(v);
    expect(normalizeTvPrompts('steam')).toBe('match');
  });

  it('drops unknown assignment styles and empty keys', () => {
    expect(normalizeTvAssignments({ aa: 'xbox', bb: ' nintendo ', cc: 'sega', '  ': 'xbox', dd: 3 })).toEqual({
      aa: 'xbox',
      bb: 'nintendo',
    });
    expect(normalizeTvAssignments(undefined)).toEqual({});
    expect(normalizeTvAssignments(null)).toEqual({});
  });
});
