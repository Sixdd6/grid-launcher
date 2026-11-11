import { describe, expect, it } from 'vitest';
import { faceGlyph, promptSet } from './prompts';

describe('promptSet', () => {
  it('Match: an Xbox-layout pad shows Xbox, or PlayStation on a PlayStation pad (item 67)', () => {
    expect(promptSet('match', 'xbox', 'xbox')).toBe('xbox');
    expect(promptSet('match', 'xbox', 'ps')).toBe('ps');
    expect(promptSet('match', 'xbox', '8bitdo')).toBe('xbox');
  });

  it('Match: a Nintendo-layout pad shows Nintendo, or 8BitDo Pro on an 8BitDo (item 68)', () => {
    expect(promptSet('match', 'nintendo', 'nintendo')).toBe('nintendo');
    expect(promptSet('match', 'nintendo', '8bitdo')).toBe('8bitdo');
  });

  it('a set chosen by hand wins for every controller (item 69)', () => {
    for (const set of ['xbox', 'ps', 'nintendo', '8bitdo'] as const) {
      expect(promptSet(set, 'nintendo', 'ps')).toBe(set);
      expect(promptSet(set, 'xbox', '8bitdo')).toBe(set);
    }
  });

  it('no controller: Xbox (item 56)', () => {
    expect(promptSet('match', 'xbox', 'xbox')).toBe('xbox');
  });
});

describe('faceGlyph', () => {
  it('draws each role as itself when the assignment matches the set', () => {
    for (const b of ['A', 'B', 'X', 'Y', 'LB', 'MENU'] as const) {
      expect(faceGlyph(b, 'xbox', 'xbox')).toBe(b);
      expect(faceGlyph(b, 'xbox', 'ps')).toBe(b);
      expect(faceGlyph(b, 'nintendo', 'nintendo')).toBe(b);
      expect(faceGlyph(b, 'nintendo', '8bitdo')).toBe(b);
    }
  });

  it('a Nintendo pad in Xbox style selects with its bottom button, printed B', () => {
    expect(faceGlyph('A', 'xbox', 'nintendo')).toBe('B');
    expect(faceGlyph('B', 'xbox', 'nintendo')).toBe('A');
    expect(faceGlyph('X', 'xbox', '8bitdo')).toBe('Y');
    expect(faceGlyph('Y', 'xbox', '8bitdo')).toBe('X');
  });

  it('an Xbox pad in Nintendo style selects with its right button, printed B', () => {
    expect(faceGlyph('A', 'nintendo', 'xbox')).toBe('B');
    expect(faceGlyph('B', 'nintendo', 'ps')).toBe('A');
    expect(faceGlyph('X', 'nintendo', 'xbox')).toBe('Y');
  });

  it('only the four face buttons move', () => {
    for (const b of ['LB', 'RB', 'LT', 'RT', 'VIEW', 'MENU', 'RS', 'LS', 'DPAD'] as const) {
      expect(faceGlyph(b, 'xbox', 'nintendo')).toBe(b);
      expect(faceGlyph(b, 'nintendo', 'xbox')).toBe(b);
    }
  });
});
