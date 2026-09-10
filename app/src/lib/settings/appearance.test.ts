import { describe, expect, it } from 'vitest';
import { FADE_DEFAULT } from '../theme';
import { backgroundEnabled, CARD_SIZE_VIEWS, fadeForToggle, rememberFade } from './appearance';

describe('backgroundEnabled', () => {
  it('is off exactly at fade 0', () => {
    expect(backgroundEnabled(0)).toBe(false);
    expect(backgroundEnabled(1)).toBe(true);
    expect(backgroundEnabled(60)).toBe(true);
  });
});

describe('rememberFade', () => {
  it('keeps the last non-zero value and ignores zero', () => {
    expect(rememberFade(40, 25)).toBe(40);
    expect(rememberFade(0, 40)).toBe(40);
  });
});

describe('fadeForToggle', () => {
  it('off writes 0; on restores the remembered value', () => {
    expect(fadeForToggle(false, 40)).toBe(0);
    expect(fadeForToggle(true, 40)).toBe(40);
  });
  it('on with nothing remembered uses the design default', () => {
    expect(fadeForToggle(true, 0)).toBe(FADE_DEFAULT);
  });
});

describe('CARD_SIZE_VIEWS', () => {
  it('lists the two grids with their ids', () => {
    expect(CARD_SIZE_VIEWS.map((v) => v.view)).toEqual(['library', 'server']);
    expect(CARD_SIZE_VIEWS.map((v) => v.testId)).toEqual(['card-size-library', 'card-size-server']);
    expect(CARD_SIZE_VIEWS.map((v) => v.label)).toEqual(['Library cards', 'Server cards']);
  });
});
