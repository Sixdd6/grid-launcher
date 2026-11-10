import { describe, expect, it } from 'vitest';
import { buttonForKey } from './buttons';

describe('buttonForKey (item 71)', () => {
  it.each([
    ['ArrowUp', 'UP'],
    ['ArrowDown', 'DOWN'],
    ['ArrowLeft', 'LEFT'],
    ['ArrowRight', 'RIGHT'],
    ['Enter', 'A'],
    [' ', 'A'],
    ['Escape', 'B'],
    ['Backspace', 'B'],
    ['x', 'X'],
    ['Y', 'Y'],
    ['q', 'LB'],
    ['e', 'RB'],
    ['z', 'LT'],
    ['r', 'RT'],
    ['m', 'MENU'],
    ['v', 'VIEW'],
  ])('%s → %s', (key, b) => {
    expect(buttonForKey(key)).toBe(b);
  });

  it('G is not Guide (ruling 5), and chords are not buttons', () => {
    expect(buttonForKey('g')).toBeNull();
    expect(buttonForKey('r', { ctrl: true })).toBeNull();
    expect(buttonForKey('Tab')).toBeNull();
  });
});
