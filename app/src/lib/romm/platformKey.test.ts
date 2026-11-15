import { describe, expect, it } from 'vitest';
import { installedPlatformSlug, isEmulatorsShelf, platformKeyer, platformRailKey, serverPlatformSlug } from './platformKey';
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

describe('helpers', () => {
  it('builds platform rail keys', () => {
    expect(platformRailKey('snes')).toBe('platform:snes');
  });
  it('spots the Emulators shelf', () => {
    expect(isEmulatorsShelf(platform({ slug: 'emulators', name: 'Emulators', display_name: 'Emulators' }))).toBe(true);
    expect(isEmulatorsShelf(platform())).toBe(false);
  });
});
