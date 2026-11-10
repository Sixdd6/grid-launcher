import { describe, expect, it, vi } from 'vitest';
import {
  clampVolume,
  debounced,
  fromElementVolume,
  stepVolume,
  toElementVolume,
  VOLUME_DEFAULT,
  volumeLabel,
} from './videoVolume';

describe('clampVolume', () => {
  it('keeps a whole percentage inside 0–100', () => {
    expect(clampVolume(0)).toBe(0);
    expect(clampVolume(50)).toBe(50);
    expect(clampVolume(100)).toBe(100);
    expect(clampVolume(140)).toBe(100);
    expect(clampVolume(-5)).toBe(0);
    expect(clampVolume(33.6)).toBe(34);
  });

  it('is the default (50) for anything that is not a number', () => {
    expect(VOLUME_DEFAULT).toBe(50);
    expect(clampVolume(undefined)).toBe(50);
    expect(clampVolume(Number.NaN)).toBe(50);
    expect(clampVolume('80')).toBe(50);
  });
});

describe('stepVolume', () => {
  it('moves 10 points per press and stops at the ends', () => {
    expect(stepVolume(50, 1)).toBe(60);
    expect(stepVolume(50, -1)).toBe(40);
    expect(stepVolume(95, 1)).toBe(100);
    expect(stepVolume(100, 1)).toBe(100);
    expect(stepVolume(4, -1)).toBe(0);
    expect(stepVolume(0, -1)).toBe(0);
  });
});

describe('element volume', () => {
  it('maps the percentage to 0–1 and back', () => {
    expect(toElementVolume(50)).toBe(0.5);
    expect(toElementVolume(150)).toBe(1);
    expect(fromElementVolume(0.3)).toBe(30);
    expect(fromElementVolume(1)).toBe(100);
  });
});

describe('volumeLabel', () => {
  it('reads "Volume N%"', () => {
    expect(volumeLabel(50)).toBe('Volume 50%');
    expect(volumeLabel(0)).toBe('Volume 0%');
  });
});

describe('debounced', () => {
  it('runs once, after the last call settles', () => {
    vi.useFakeTimers();
    try {
      const fn = vi.fn();
      const run = debounced(fn, 500);
      run();
      vi.advanceTimersByTime(300);
      run();
      vi.advanceTimersByTime(499);
      expect(fn).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1);
      expect(fn).toHaveBeenCalledTimes(1);
      run();
      vi.advanceTimersByTime(500);
      expect(fn).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });
});
