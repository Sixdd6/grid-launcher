import { describe, expect, it } from 'vitest';
import { FADE_DEFAULT, FADE_MAX } from '../theme';
import { backgroundEnabled, CARD_SIZE_VIEWS, fadeForToggle, rememberFade } from './appearance';

describe('backgroundEnabled', () => {
  it('is off exactly at a full fade', () => {
    expect(backgroundEnabled(100)).toBe(false);
    expect(backgroundEnabled(99)).toBe(true);
    expect(backgroundEnabled(0)).toBe(true);
  });
});

describe('rememberFade', () => {
  it('keeps the last value short of off and ignores a full fade', () => {
    expect(rememberFade(40, 25)).toBe(40);
    expect(rememberFade(0, 25)).toBe(0);
    expect(rememberFade(100, 40)).toBe(40);
  });
});

describe('fadeForToggle', () => {
  it('off writes a full fade; on restores the remembered value', () => {
    expect(fadeForToggle(false, 40)).toBe(FADE_MAX);
    expect(fadeForToggle(true, 40)).toBe(40);
    expect(fadeForToggle(true, 0)).toBe(0);
  });
  it('on with nothing short of off remembered uses the design default', () => {
    expect(fadeForToggle(true, FADE_MAX)).toBe(FADE_DEFAULT);
  });
});

describe('CARD_SIZE_VIEWS', () => {
  it('lists the two grids with their ids', () => {
    expect(CARD_SIZE_VIEWS.map((v) => v.view)).toEqual(['library', 'server']);
    expect(CARD_SIZE_VIEWS.map((v) => v.testId)).toEqual(['card-size-library', 'card-size-server']);
    expect(CARD_SIZE_VIEWS.map((v) => v.label)).toEqual(['Library cards', 'Server cards']);
  });
});
