import { describe, expect, it } from 'vitest';
import { leftStickDirections, pageScroll, rightStickActive, rightStickScroll } from './sticks';

describe('left stick (item 35)', () => {
  it('moves past half its travel; Y is positive up', () => {
    expect(leftStickDirections(0.4, 0.4)).toEqual([]);
    expect(leftStickDirections(0.6, 0)).toEqual(['RIGHT']);
    expect(leftStickDirections(-0.6, 0)).toEqual(['LEFT']);
    expect(leftStickDirections(0, 0.6)).toEqual(['UP']);
    expect(leftStickDirections(0, -0.6)).toEqual(['DOWN']);
    expect(leftStickDirections(0.9, -0.9)).toEqual(['RIGHT', 'DOWN']);
  });
});

describe('right stick (item 36)', () => {
  it('has a 0.25 dead zone', () => {
    expect(rightStickActive(0.25)).toBe(false);
    expect(rightStickActive(-0.3)).toBe(true);
    expect(rightStickScroll(0.2, 16, 20)).toBe(0);
  });

  it('scrolls proportionally to the tilt; tilted down scrolls down', () => {
    const full = rightStickScroll(-1, 1000 / 60, 20);
    expect(full).toBeCloseTo(22);
    expect(rightStickScroll(-0.5, 1000 / 60, 20)).toBeCloseTo(11);
    expect(rightStickScroll(1, 1000 / 60, 20)).toBeCloseTo(-22);
    // Twice the time, twice the distance; twice the root font, twice too.
    expect(rightStickScroll(-1, 2000 / 60, 20)).toBeCloseTo(44);
    expect(rightStickScroll(-1, 1000 / 60, 40)).toBeCloseTo(44);
  });
});

describe('Page Up / Page Down (items 114, 25)', () => {
  it('scrolls 70% of the view, gliding, or jumping under reduced motion', () => {
    expect(pageScroll(true, 1000, false)).toEqual({ top: 700, behavior: 'smooth' });
    expect(pageScroll(false, 1000, false)).toEqual({ top: -700, behavior: 'smooth' });
    expect(pageScroll(true, 1000, true)).toEqual({ top: 700, behavior: 'auto' });
  });
});
