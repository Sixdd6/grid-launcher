import { describe, expect, it } from 'vitest';
import { CARD_CELL_ASPECT } from './size';
import { cellAspect, MAX_CELL_ASPECT, MIN_CELL_ASPECT, MIN_SAMPLES } from './shape';

const many = (ratio: number, n = MIN_SAMPLES) => Array.from({ length: n }, () => ratio);

describe('cellAspect', () => {
  it('stays 3:4 until enough covers have loaded', () => {
    expect(cellAspect([])).toBe(CARD_CELL_ASPECT);
    expect(cellAspect(many(1, MIN_SAMPLES - 1))).toBe(CARD_CELL_ASPECT);
  });

  it('makes a platform of square covers square', () => {
    expect(cellAspect(many(360 / 309))).toBe(MAX_CELL_ASPECT);
    expect(cellAspect(many(1))).toBe(1);
  });

  it('makes a platform of tall covers taller, down to the limit', () => {
    expect(cellAspect(many(297 / 480))).toBe(0.62);
    expect(cellAspect(many(0.4))).toBe(MIN_CELL_ASPECT);
  });

  it('keeps near-portrait platforms at exactly 3:4', () => {
    expect(cellAspect(many(0.72))).toBe(CARD_CELL_ASPECT);
    expect(cellAspect(many(0.78))).toBe(CARD_CELL_ASPECT);
  });

  it('takes the median, so a few odd covers do not move it', () => {
    expect(cellAspect([1, 1, 1, 1, 1, 0.62, 0.62])).toBe(1);
    expect(cellAspect([0.62, 0.62, 0.62, 0.62, 1, 1])).toBe(0.62);
  });

  it('ignores broken measurements', () => {
    expect(cellAspect([...many(1), Number.NaN, 0, -1, Infinity])).toBe(1);
  });
});
