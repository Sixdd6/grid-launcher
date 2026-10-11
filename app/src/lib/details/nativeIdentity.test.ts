// User bug 2026-10-10: a native PC game on a custom-named platform (RomM
// platform 52 — slug `win`, fs_slug `win-fangames-ports`, display name
// "PC Fangames & Ports") was treated as emulated: Details asked for an
// emulator and had no Native Game Settings. Nativeness follows the server
// platform's slug, not its display name.
import { describe, expect, it } from 'vitest';
import type { InstalledGame, RomDetail } from '../api';
import { compatToolNotice, installLabel, isNativePlatform } from './actions';
import { launchTargetLine } from './header';
import { fromInstalled, mergeDetail } from './subject';
import { needsEmulator } from '../emulators/defaults';
import { showsEmulatorChip } from '../server/header';

const NAME = 'PC Fangames & Ports';

describe('nativeness follows the platform slug', () => {
  it('is native for slug win / win9x / linux whatever the display name', () => {
    expect(isNativePlatform('win', NAME)).toBe(true);
    expect(isNativePlatform(' WIN ', 'My PC Shelf')).toBe(true);
    expect(isNativePlatform('win9x', 'Old PC')).toBe(true);
    expect(isNativePlatform('linux', 'Steam Deck Ports')).toBe(true);
  });

  it('is not native for an emulated slug, and the fs_slug is never the identity', () => {
    expect(isNativePlatform('dos', 'PC (DOS)')).toBe(false);
    expect(isNativePlatform('snes', 'win-fangames-ports')).toBe(false);
  });

  it('falls back to the display-name rule when the slug is blank', () => {
    expect(isNativePlatform('', 'Windows')).toBe(true);
    expect(isNativePlatform('', NAME)).toBe(false);
  });
});

describe('Details for a custom-named Windows platform', () => {
  it('shows the native install label and states no emulator launch target', () => {
    expect(installLabel('win', NAME)).toBe('Install App');
    const defaults = { default_emulators: {}, retroarch_cores: {}, launch_args: '' };
    // '' = no launch-target line: no "No default emulator" prompt.
    expect(launchTargetLine(defaults, NAME, 'win')).toBe('');
    expect(launchTargetLine(null, NAME, 'win')).toBe('');
  });

  it('offers the compatibility-tool picker (a Windows game, not a Linux one)', () => {
    expect(compatToolNotice('win', NAME)).toBe('');
    expect(compatToolNotice('linux', 'Steam Deck Ports')).toBe('Linux games run directly.');
  });

  it('reads the slug from the registry row, or from the ROM detail when the row has none', () => {
    const row = { title: 'Tamriel Rebuilt', platform: NAME, platform_slug: 'win', rom_id: 52001 } as InstalledGame;
    const subject = fromInstalled({ ...row, screenshot_urls: '', fanart_urls: '' });
    expect(subject.platformSlug).toBe('win');
    expect(isNativePlatform(subject.platformSlug, subject.platformName)).toBe(true);

    // A row from before registry v8 (not yet backfilled): the ROM detail fills it.
    const legacy = fromInstalled({ ...row, platform_slug: '', screenshot_urls: '', fanart_urls: '' });
    expect(isNativePlatform(legacy.platformSlug, legacy.platformName)).toBe(false);
    const merged = mergeDetail(legacy, {
      platform_slug: 'win',
      cover_small_path: '',
      cover_large_path: '',
      screenshot_urls: [],
      fanart_urls: [],
      description: '',
      rating: '',
      genres: '',
    } as unknown as RomDetail);
    expect(merged.platformSlug).toBe('win');
    expect(isNativePlatform(merged.platformSlug, merged.platformName)).toBe(true);
  });
});

describe('Server and Emulators panels for a custom-named Windows platform', () => {
  it('shows no emulator chip and offers no default emulator', () => {
    expect(showsEmulatorChip(NAME, 'win')).toBe(false);
    expect(needsEmulator({ display_name: NAME, name: 'Windows', slug: 'win' })).toBe(false);
    expect(needsEmulator({ display_name: NAME, name: NAME, slug: 'win' })).toBe(false);
  });
});
