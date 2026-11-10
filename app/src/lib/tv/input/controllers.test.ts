import { describe, expect, it } from 'vitest';
import type { PadButton } from '../../api';
import { assignmentFor, guessAssign, logicalButton, padBrand, padDisplayName } from './controllers';

const pad = (name: string, vendor: number | null = null) => ({ name, vendor, key: 'k' });

describe('padBrand', () => {
  it.each([
    [pad('Xbox Wireless Controller', 0x045e), 'xbox'],
    [pad('Sony Interactive Entertainment Wireless Controller', 0x054c), 'ps'],
    [pad('DualSense Wireless Controller'), 'ps'],
    [pad('PS4 Controller (DualShock 4)'), 'ps'],
    [pad('Pro Controller', 0x057e), 'nintendo'],
    [pad('Nintendo Switch Pro Controller'), 'nintendo'],
    [pad('Joy-Con (L/R)'), 'nintendo'],
    [pad('8BitDo Pro 2', 0x2dc8), '8bitdo'],
    [pad('8BitDo Ultimate Wireless'), '8bitdo'],
    // An 8BitDo in X-input mode reports as Microsoft.
    [pad('Xbox 360 Controller', 0x045e), 'xbox'],
  ] as const)('%o → %s', (p, brand) => {
    expect(padBrand(p)).toBe(brand);
  });
});

describe('guessAssign (items 51–52)', () => {
  it.each([
    [pad('Pro Controller', 0x057e), 'nintendo'],
    [pad('Joy-Con (L/R)'), 'nintendo'],
    [pad('Switch Online SNES'), 'nintendo'],
    [pad('8BitDo Pro 2', 0x2dc8), 'nintendo'],
    [pad('8BitDo SN30 Pro'), 'nintendo'],
    [pad('8BitDo Lite 2'), 'nintendo'],
    [pad('8BitDo Zero 2 gamepad'), 'nintendo'],
    [pad('8BitDo Micro'), 'nintendo'],
    [pad('8BitDo Ultimate Wireless'), 'xbox'],
    // The model must be outside the parentheses.
    [pad('8BitDo Ultimate (pro firmware)'), 'xbox'],
    [pad('Xbox Wireless Controller', 0x045e), 'xbox'],
    [pad('DualSense Wireless Controller', 0x054c), 'xbox'],
    [pad('Generic USB Joystick'), 'xbox'],
  ] as const)('%o → %s', (p, style) => {
    expect(guessAssign(p)).toBe(style);
  });
});

describe('assignmentFor (item 53)', () => {
  it('a stored choice wins for that controller only', () => {
    const nin = { ...pad('Pro Controller', 0x057e), key: 'nin' };
    expect(assignmentFor(nin, {})).toBe('nintendo');
    expect(assignmentFor(nin, { nin: 'xbox' })).toBe('xbox');
    expect(assignmentFor(nin, { other: 'xbox' })).toBe('nintendo');
  });
});

describe('padDisplayName (item 38)', () => {
  it('drops parenthesised parts', () => {
    expect(padDisplayName('Xbox Wireless Controller (model 1914)')).toBe('Xbox Wireless Controller');
    expect(padDisplayName('Joy-Con (L/R)')).toBe('Joy-Con');
  });
  it('says Controller when nothing is left', () => {
    expect(padDisplayName('(unknown)')).toBe('Controller');
    expect(padDisplayName('  ')).toBe('Controller');
  });
});

describe('logicalButton', () => {
  it('Xbox style: bottom A, right B, left X, top Y (item 49)', () => {
    expect(['south', 'east', 'west', 'north'].map((b) => logicalButton(b as PadButton, 'xbox'))).toEqual(['A', 'B', 'X', 'Y']);
  });
  it('Nintendo style: right A, bottom B, top X, left Y (item 50)', () => {
    expect(['east', 'south', 'north', 'west'].map((b) => logicalButton(b as PadButton, 'nintendo'))).toEqual(['A', 'B', 'X', 'Y']);
  });
  it('only the face buttons move (item 55)', () => {
    const fixed: [PadButton, string][] = [
      ['left_bumper', 'LB'],
      ['right_bumper', 'RB'],
      ['left_trigger', 'LT'],
      ['right_trigger', 'RT'],
      ['select', 'VIEW'],
      ['start', 'MENU'],
      ['dpad_up', 'UP'],
      ['dpad_down', 'DOWN'],
      ['dpad_left', 'LEFT'],
      ['dpad_right', 'RIGHT'],
    ];
    for (const [b, l] of fixed) {
      expect(logicalButton(b, 'xbox')).toBe(l);
      expect(logicalButton(b, 'nintendo')).toBe(l);
    }
  });
  it('Guide and the stick presses do nothing (ruling 5)', () => {
    for (const b of ['guide', 'left_stick', 'right_stick'] as PadButton[]) {
      expect(logicalButton(b, 'xbox')).toBeNull();
      expect(logicalButton(b, 'nintendo')).toBeNull();
    }
  });
});
