import { describe, expect, it } from 'vitest';
import { SAMPLE_COUNT } from './ring';
import { linePath, sharedMax, sparklinePaths } from './sparkline';

describe('sharedMax', () => {
  it('is the largest value across both series', () => {
    expect(sharedMax([{ net: 3, disk: 9 }, { net: 12, disk: 1 }])).toBe(12);
    expect(sharedMax([{ net: 3, disk: 90 }, { net: 12, disk: 1 }])).toBe(90);
  });

  it('is at least 1 so a flat line never divides by zero', () => {
    expect(sharedMax([])).toBe(1);
    expect(sharedMax([{ net: 0, disk: 0 }])).toBe(1);
    expect(sharedMax([{ net: 0.25, disk: 0 }])).toBe(1);
  });
});

describe('linePath', () => {
  const box = { width: 20, height: 10 };

  it('is empty with no values', () => {
    expect(linePath([], 3, box, 10)).toBe('');
  });

  it('anchors the newest sample at the right edge and leaves 1px of padding top and bottom', () => {
    // capacity 3 over 20px → one step is 10px. Two values fill the last two
    // slots: x = 10 and x = 20. 0 sits 1px above the bottom, max 1px below
    // the top.
    expect(linePath([0, 10], 3, box, 10)).toBe('M 10 9 L 20 1');
  });

  it('draws a full buffer from the left edge', () => {
    expect(linePath([5, 5, 5], 3, box, 10)).toBe('M 0 5 L 10 5 L 20 5');
  });

  it('renders a single sample as a zero-length segment (a dot with round caps)', () => {
    expect(linePath([10], 3, box, 10)).toBe('M 20 1 L 20 1');
  });

  it('rounds coordinates to two decimals', () => {
    const values = new Array<number>(SAMPLE_COUNT).fill(0);
    const d = linePath(values, SAMPLE_COUNT, { width: 120, height: 38 }, 1);
    expect(d.startsWith('M 0 37 L 2.03 37 L 4.07 37')).toBe(true);
    expect(d.endsWith('L 120 37')).toBe(true);
  });

  it('clamps a value above max to the top', () => {
    expect(linePath([50], 1, box, 10)).toBe('M 0 1 L 0 1');
  });
});

describe('sparklinePaths', () => {
  it('draws both series on one shared scale', () => {
    const box = { width: 20, height: 10 };
    const paths = sparklinePaths([{ net: 10, disk: 5 }, { net: 0, disk: 10 }], box, 2);
    expect(paths.max).toBe(10);
    expect(paths.net).toBe('M 0 1 L 20 9');
    expect(paths.disk).toBe('M 0 5 L 20 1');
  });

  it('defaults to the 60-sample capacity and empty paths for no samples', () => {
    const paths = sparklinePaths([], { width: 120, height: 38 });
    expect(paths).toEqual({ net: '', disk: '', max: 1 });
  });
});
