import { describe, expect, it } from 'vitest';
import { segmentKeyTarget, segmentTabIndex } from './segmented';

describe('segmentKeyTarget', () => {
  it('moves forward with ArrowRight and ArrowDown, wrapping at the end', () => {
    expect(segmentKeyTarget('ArrowRight', 0, 2)).toBe(1);
    expect(segmentKeyTarget('ArrowDown', 0, 2)).toBe(1);
    expect(segmentKeyTarget('ArrowRight', 1, 2)).toBe(0);
  });

  it('moves back with ArrowLeft and ArrowUp, wrapping at the start', () => {
    expect(segmentKeyTarget('ArrowLeft', 1, 2)).toBe(0);
    expect(segmentKeyTarget('ArrowUp', 1, 3)).toBe(0);
    expect(segmentKeyTarget('ArrowLeft', 0, 3)).toBe(2);
  });

  it('jumps to the ends with Home and End', () => {
    expect(segmentKeyTarget('Home', 2, 3)).toBe(0);
    expect(segmentKeyTarget('End', 0, 3)).toBe(2);
  });

  it('ignores other keys and empty switches', () => {
    expect(segmentKeyTarget('Tab', 0, 2)).toBeNull();
    expect(segmentKeyTarget(' ', 0, 2)).toBeNull();
    expect(segmentKeyTarget('ArrowRight', 0, 0)).toBeNull();
  });

  it('treats an out-of-range index as the first option', () => {
    expect(segmentKeyTarget('ArrowRight', -1, 2)).toBe(1);
  });
});

describe('segmentTabIndex', () => {
  it('is the chosen option', () => {
    expect(segmentTabIndex(['always', 'hover'], 'hover')).toBe(1);
  });

  it('falls back to the first option for an unknown value', () => {
    expect(segmentTabIndex(['always', 'hover'], 'other')).toBe(0);
  });
});
