import { describe, expect, it } from 'vitest';
import { promptSet } from './prompts';

describe('promptSet', () => {
  it('Match: Xbox style shows Xbox, or PlayStation on a PlayStation pad (item 67)', () => {
    expect(promptSet('match', 'xbox', 'xbox')).toBe('xbox');
    expect(promptSet('match', 'xbox', 'ps')).toBe('ps');
    expect(promptSet('match', 'xbox', 'nintendo')).toBe('xbox');
    expect(promptSet('match', 'xbox', '8bitdo')).toBe('xbox');
  });

  it('Match: Nintendo style shows Nintendo, or 8BitDo Pro on an 8BitDo (item 68)', () => {
    expect(promptSet('match', 'nintendo', 'nintendo')).toBe('nintendo');
    expect(promptSet('match', 'nintendo', '8bitdo')).toBe('8bitdo');
    expect(promptSet('match', 'nintendo', 'xbox')).toBe('nintendo');
    expect(promptSet('match', 'nintendo', 'ps')).toBe('nintendo');
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
