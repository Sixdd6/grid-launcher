import { describe, expect, it } from 'vitest';
import { moveFocus } from './grid';

describe('moveFocus (4 columns, 10 items)', () => {
  it('moves within a row and clamps at edges', () => {
    expect(moveFocus(0, 'right', 4, 10)).toBe(1);
    expect(moveFocus(3, 'right', 4, 10)).toBe(3); // row edge: clamp, no reading-order flow
  });
  it('clamps at row edges without wrapping', () => {
    expect(moveFocus(3, 'left', 4, 10)).toBe(2);
    expect(moveFocus(0, 'left', 4, 10)).toBe(0);
    expect(moveFocus(9, 'right', 4, 10)).toBe(9);
  });
  it('moves between rows and clamps on the last partial row', () => {
    expect(moveFocus(1, 'down', 4, 10)).toBe(5);
    expect(moveFocus(7, 'down', 4, 10)).toBe(9); // row below has only items 8,9 -> clamp to last
    expect(moveFocus(5, 'up', 4, 10)).toBe(1);
    expect(moveFocus(1, 'up', 4, 10)).toBe(1);
  });
});
