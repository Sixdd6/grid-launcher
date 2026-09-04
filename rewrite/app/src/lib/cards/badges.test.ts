import { describe, expect, it } from 'vitest';
import { cardBadges, cloudPlatformSet, shortPlatformName, UPDATE_TAG_TEXT } from './badges';

describe('cardBadges (D-UI-9)', () => {
  const cloud = new Set(['SNES']);

  it('shows the installed dot and the platform chip for a plain installed game', () => {
    expect(
      cardBadges({ platform: 'SNES', installed: true, updateLabel: null, cloudPlatforms: cloud }),
    ).toEqual({ installed: true, update: false, cloud: true, platform: 'SNES' });
  });

  it('shows the UPDATE tag only when the updates store has a label for the rom', () => {
    expect(
      cardBadges({
        platform: 'SNES',
        installed: true,
        updateLabel: 'Update to v1.1.0',
        cloudPlatforms: cloud,
      }).update,
    ).toBe(true);
  });

  it('never shows the UPDATE tag for a game that is not installed', () => {
    // The server-side update set only ever covers installed rows, but the
    // Server grid renders both, and a tag on an uninstalled card would read
    // as "your copy is stale" about a copy that does not exist.
    expect(
      cardBadges({
        platform: 'SNES',
        installed: false,
        updateLabel: 'Update',
        cloudPlatforms: cloud,
      }).update,
    ).toBe(false);
  });

  it('drops the cloud icon for a platform with no cloud sync configured', () => {
    expect(
      cardBadges({ platform: 'Arcade', installed: true, updateLabel: null, cloudPlatforms: cloud }).cloud,
    ).toBe(false);
  });

  it('matches the cloud platform case- and space-insensitively', () => {
    expect(
      cardBadges({
        platform: '  snes ',
        installed: true,
        updateLabel: null,
        cloudPlatforms: cloud,
      }).cloud,
    ).toBe(true);
  });

  it('keeps the tag text fixed at UPDATE', () => {
    expect(UPDATE_TAG_TEXT).toBe('UPDATE');
  });
});

describe('shortPlatformName', () => {
  it('leaves a name that already fits alone', () => {
    expect(shortPlatformName('Arcade')).toBe('Arcade');
    expect(shortPlatformName('Nintendo 64')).toBe('Nintendo 64');
  });
  it('initialises a long name, keeping digit runs whole', () => {
    expect(shortPlatformName('Super Nintendo Entertainment System')).toBe('SNES');
    expect(shortPlatformName('PlayStation 3')).toBe('PS3');
    expect(shortPlatformName('Microsoft Xbox 360')).toBe('MX360');
  });
  it('falls back to a truncation when there is nothing to initialise', () => {
    expect(shortPlatformName('a very long lowercase platform')).toBe('a very long…');
  });
  it('is blank for a blank name rather than an ellipsis', () => {
    expect(shortPlatformName('   ')).toBe('');
  });
});

describe('cloudPlatformSet', () => {
  it('keeps only platforms that actually name a default emulator', () => {
    const set = cloudPlatformSet({ SNES: 'RetroArch', Arcade: '', 'PlayStation 3': 'RPCS3' });
    expect([...set].sort()).toEqual(['playstation 3', 'snes']);
  });
});
