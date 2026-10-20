import { describe, expect, it } from 'vitest';
import { leavesLetterbox } from './letterbox';

describe('leavesLetterbox', () => {
  it('is false for a 3:4 cover in a 3:4 frame, at any size', () => {
    expect(leavesLetterbox(264, 352, 150, 200)).toBe(false);
    expect(leavesLetterbox(600, 800, 210, 280)).toBe(false);
  });

  it('ignores a sub-pixel difference', () => {
    // 263 × 352 in 150 × 200 draws 149.4 px wide: 0.6 px of rounding.
    expect(leavesLetterbox(263, 352, 150, 200)).toBe(false);
  });

  it('is true for square and wide art', () => {
    expect(leavesLetterbox(512, 512, 150, 200)).toBe(true);
    expect(leavesLetterbox(640, 360, 150, 200)).toBe(true);
  });

  it('is true for art taller than the frame', () => {
    expect(leavesLetterbox(300, 600, 150, 200)).toBe(true);
  });

  it('is true when a size is unknown', () => {
    expect(leavesLetterbox(0, 0, 150, 200)).toBe(true);
    expect(leavesLetterbox(264, 352, 0, 0)).toBe(true);
  });
});
