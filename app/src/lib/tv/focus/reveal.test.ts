import { describe, expect, it } from 'vitest';
import { readStep, revealX, revealY, type VerticalInput } from './reveal';

const rem = 20;
const V = (o: Partial<VerticalInput>): VerticalInput => ({
  areaTop: 0,
  areaBottom: 500,
  itemTop: 100,
  itemBottom: 200,
  rem,
  scrollTop: 0,
  scrollHeight: 2000,
  clientHeight: 500,
  first: false,
  last: false,
  ...o,
});

describe('revealY', () => {
  it('leaves an item already 1.2rem inside the area', () => {
    expect(revealY(V({}))).toBeNull();
  });

  it('keeps the item 1.2rem (24px) inside the bottom edge (item 111)', () => {
    // Item bottom 520 + 24 = 544 is 44 past 500.
    expect(revealY(V({ itemTop: 420, itemBottom: 520 }))).toBe(44);
  });

  it('keeps the item 1.2rem inside the top edge', () => {
    expect(revealY(V({ scrollTop: 300, itemTop: 10, itemBottom: 110 }))).toBe(300 + 10 - 24);
  });

  it('lines an item taller than the area up with its top', () => {
    // 100..800 in a 500 area: top - pad = 76, so scroll 76 (not 324).
    expect(revealY(V({ itemTop: 100, itemBottom: 800 }))).toBe(76);
  });

  it('an item taller than the area that starts below lines up with its top', () => {
    expect(revealY(V({ itemTop: 600, itemBottom: 1300 }))).toBe(600 - 24);
  });

  it('the first item scrolls to the very top, the last to the very bottom (item 112)', () => {
    expect(revealY(V({ scrollTop: 300, itemTop: 40, itemBottom: 90, first: true }))).toBe(0);
    expect(revealY(V({ itemTop: 420, itemBottom: 470, last: true }))).toBe(1500);
  });

  it('a Discover shelf brings its title to the top; the first shelf to the very top (item 108)', () => {
    expect(revealY(V({ scrollTop: 100, shelfTop: 250 }))).toBe(350);
    expect(revealY(V({ scrollTop: 400, shelfTop: null }))).toBe(0);
  });

  it('clamps to the scroll range', () => {
    expect(revealY(V({ scrollTop: 1400, shelfTop: 900 }))).toBe(1500);
  });
});

describe('revealX', () => {
  const base = { trackLeft: 0, trackRight: 1000, edge: 24, scrollLeft: 200, scrollWidth: 3000, clientWidth: 1000 };

  it('keeps the focused cover one row padding inside the right edge (item 110, ruling 3)', () => {
    expect(revealX({ ...base, itemLeft: 900, itemRight: 990 })).toBe(200 + 990 - 1000 + 24);
  });

  it('and inside the left edge', () => {
    expect(revealX({ ...base, itemLeft: 10, itemRight: 100 })).toBe(200 + 10 - 24);
  });

  it('leaves a cover already inside', () => {
    expect(revealX({ ...base, itemLeft: 400, itemRight: 500 })).toBeNull();
  });

  it('never scrolls before the start', () => {
    expect(revealX({ ...base, scrollLeft: 5, itemLeft: 10, itemRight: 100 })).toBe(0);
  });
});

describe('readStep', () => {
  it('scrolls a long summary down by at most 6rem while more is below', () => {
    expect(readStep('down', 0, 2000, 0, 500, rem)).toBe(120);
    expect(readStep('down', 0, 520, 0, 500, rem)).toBe(44);
    expect(readStep('down', 0, 400, 0, 500, rem)).toBeNull();
  });

  it('and up while more is above', () => {
    expect(readStep('up', -300, 400, 0, 500, rem)).toBe(-120);
    expect(readStep('up', 10, 400, 0, 500, rem)).toBeNull();
  });
});
