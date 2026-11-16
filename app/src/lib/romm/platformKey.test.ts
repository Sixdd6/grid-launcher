import { describe, expect, it } from 'vitest';
import { installedPlatformSlug, isEmulatorsShelf, platformKeyer, platformRailKey, railSlugs, serverPlatformSlug } from './platformKey';
import { installed, platform } from './fixtures.test-util';

describe('serverPlatformSlug', () => {
  it('is the server slug', () => {
    expect(serverPlatformSlug(platform({ slug: 'n64' }))).toBe('n64');
  });
  it('falls back to the label token when the server sent no slug', () => {
    expect(serverPlatformSlug(platform({ slug: '', display_name: 'Sega CD / Mega-CD' }))).toBe('sega-cd-mega-cd');
  });
});

describe('installed row slug', () => {
  const servers = [
    platform({ id: 1, slug: 'snes', display_name: 'Super Famicom', name: 'Super Nintendo Entertainment System', custom_name: 'Super Famicom' }),
    platform({ id: 2, slug: 'n64', display_name: 'Nintendo 64', name: 'Nintendo 64' }),
  ];

  it('uses the row platform_slug first', () => {
    expect(installedPlatformSlug(installed({ platform: 'Whatever', platform_slug: 'gba' }), servers)).toBe('gba');
  });

  it('matches a pre-v8 row to the server platform with the same label', () => {
    expect(installedPlatformSlug(installed({ platform: 'Super Famicom', platform_slug: '' }), servers)).toBe('snes');
  });

  it('falls back to the platform name token', () => {
    expect(installedPlatformSlug(installed({ platform: 'Sega Saturn', platform_slug: '' }), servers)).toBe('sega-saturn');
  });

  it('lets a server slug win when a name token collides with it', () => {
    // An unlinked row named "N64" tokenizes to the server's slug "n64":
    // it joins that server platform's row, never a second "n64" row.
    const keyOf = platformKeyer(servers);
    expect(keyOf(installed({ platform: 'N64', platform_slug: '' }))).toBe('n64');
    expect(keyOf(installed({ platform: 'Nintendo 64', platform_slug: '' }))).toBe('n64');
  });

  it('works with no server list (RomM off)', () => {
    expect(installedPlatformSlug(installed({ platform: 'Nintendo 64', platform_slug: '' }), [])).toBe('nintendo-64');
  });
});

describe('two server platforms on one slug', () => {
  // RomM allows it: a custom-named "PC Fangames & Ports" on `win` beside Windows.
  const servers = [
    platform({ id: 1, slug: 'win', name: 'Windows', display_name: 'Windows' }),
    platform({ id: 3, slug: 'win', name: 'Windows', custom_name: 'PC Fangames & Ports', display_name: 'PC Fangames & Ports' }),
  ];

  it('gives the one with the higher id its own rail slug, whatever the list order', () => {
    expect([...railSlugs(servers).entries()]).toEqual([
      [1, 'win'],
      [3, 'win-3'],
    ]);
    expect(railSlugs([...servers].reverse()).get(1)).toBe('win');
    expect(railSlugs([...servers].reverse()).get(3)).toBe('win-3');
  });

  it('keys an installed row by the platform whose label it carries', () => {
    const keyOf = platformKeyer(servers);
    expect(keyOf(installed({ platform: 'PC Fangames & Ports', platform_slug: 'win' }))).toBe('win-3');
    expect(keyOf(installed({ platform: 'Windows', platform_slug: 'win' }))).toBe('win');
    // No label match: the slug's first platform.
    expect(keyOf(installed({ platform: 'Elsewhere', platform_slug: 'win' }))).toBe('win');
  });
});

describe('helpers', () => {
  it('builds platform rail keys', () => {
    expect(platformRailKey('snes')).toBe('platform:snes');
  });
  it('spots the Emulators shelf', () => {
    expect(isEmulatorsShelf(platform({ slug: 'emulators', name: 'Emulators', display_name: 'Emulators' }))).toBe(true);
    expect(isEmulatorsShelf(platform())).toBe(false);
  });
});
