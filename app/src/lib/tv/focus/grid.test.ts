import { describe, expect, it } from 'vitest';
import { gridColumns, gridMove } from './grid';

describe('gridColumns', () => {
  it('fits 10rem columns with at least a 1.2rem gap', () => {
    // 1000px wide at 20px rem: (1000 + 24) / (200 + 24) = 4.57 → 4.
    expect(gridColumns(1000, 200, 24)).toBe(4);
    expect(gridColumns(150, 200, 24)).toBe(1);
    expect(gridColumns(0, 200, 24)).toBe(1);
  });
});

describe('gridMove', () => {
  // 4 columns, 10 covers: rows 0–3, 4–7, 8–9.
  it('moves inside the grid', () => {
    expect(gridMove(5, 'left', 4, 10)).toBe(4);
    expect(gridMove(5, 'right', 4, 10)).toBe(6);
    expect(gridMove(5, 'up', 4, 10)).toBe(1);
    expect(gridMove(1, 'down', 4, 10)).toBe(5);
  });

  it('answers edge off every side (items 88–90)', () => {
    expect(gridMove(4, 'left', 4, 10)).toBe('edge');
    expect(gridMove(3, 'right', 4, 10)).toBe('edge');
    expect(gridMove(9, 'right', 4, 10)).toBe('edge');
    expect(gridMove(2, 'up', 4, 10)).toBe('edge');
    expect(gridMove(8, 'down', 4, 10)).toBe('edge');
  });

  it('down into a shorter last row lands on its last cover', () => {
    expect(gridMove(7, 'down', 4, 10)).toBe(9);
  });

  it('an empty grid is all edge', () => {
    expect(gridMove(0, 'down', 4, 0)).toBe('edge');
  });
});
