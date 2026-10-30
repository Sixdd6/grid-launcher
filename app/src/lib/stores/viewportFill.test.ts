import { describe, expect, it } from 'vitest';
import { FILL_MAX, fillFor } from './viewportFill.svelte';

describe('fillFor', () => {
  it('rounds the window up to 128px steps in device pixels', () => {
    expect(fillFor(3840, 2160, 1)).toEqual({ width: 3840, height: 2176, dpr: 1 });
    expect(fillFor(1920, 1080, 1)).toEqual({ width: 1920, height: 1152, dpr: 1 });
    expect(fillFor(1600, 1000, 2)).toEqual({ width: 3200, height: 2048, dpr: 2 });
  });

  it('caps each side and survives a broken pixel ratio', () => {
    expect(fillFor(10000, 6000, 1).width).toBe(FILL_MAX);
    expect(fillFor(1280, 720, Number.NaN).dpr).toBe(1);
    expect(fillFor(0, 0, 1)).toEqual({ width: 128, height: 128, dpr: 1 });
  });
});
