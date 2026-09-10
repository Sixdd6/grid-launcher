import { describe, expect, it } from 'vitest';
import {
  contentBlockReason,
  isEmulatorsPlatform,
  ps4ContentBlockReason,
  xbox360ContentBlockReason,
} from './blocked';

describe('isEmulatorsPlatform', () => {
  it('matches the synthetic Emulators platform, case-insensitively', () => {
    expect(isEmulatorsPlatform('Emulators')).toBe(true);
    expect(isEmulatorsPlatform('  emulators ')).toBe(true);
    expect(isEmulatorsPlatform('SNES')).toBe(false);
  });
});

describe('ps4ContentBlockReason', () => {
  it('is blank off a PS4 platform', () => {
    expect(ps4ContentBlockReason('SNES', false, null, false)).toBe('');
  });

  it('asks for the base game first', () => {
    expect(ps4ContentBlockReason('PlayStation 4', false, 5, true)).toBe(
      'Install the base PS4 game before applying update or DLC content.'
    );
  });

  it('names the missing rom id', () => {
    expect(ps4ContentBlockReason('PS4', true, null, true)).toBe(
      'This game is missing a ROM id, so update/DLC content cannot be downloaded.'
    );
  });

  it('names the absent content', () => {
    expect(ps4ContentBlockReason('PS4', true, 5, false)).toBe(
      'No update or DLC content is available for this PS4 game on the server.'
    );
  });

  it('is blank when everything is in place', () => {
    expect(ps4ContentBlockReason('PS4', true, 5, true)).toBe('');
  });
});

describe('xbox360ContentBlockReason', () => {
  it('asks for the install, then the rom id, then passes', () => {
    expect(xbox360ContentBlockReason(false, 5)).toBe('Game must be installed before content can be applied.');
    expect(xbox360ContentBlockReason(true, null)).toBe('Game is missing a ROM ID.');
    expect(xbox360ContentBlockReason(true, 5)).toBe('');
  });
});

describe('contentBlockReason', () => {
  it('routes a PS4 platform to the PS4 reasons', () => {
    expect(contentBlockReason('update', 'PS4', true, null, true)).toBe(
      'This game is missing a ROM id, so update/DLC content cannot be downloaded.'
    );
  });

  it('routes an Xbox 360 platform to the Xbox 360 reasons', () => {
    expect(contentBlockReason('dlc', 'Xbox 360', true, null, true)).toBe('Game is missing a ROM ID.');
  });

  it('is blank on a platform with no extra content at all', () => {
    expect(contentBlockReason('update', 'SNES', true, 5, true)).toBe('');
  });
});
