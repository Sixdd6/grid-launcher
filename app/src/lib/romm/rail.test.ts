import { describe, expect, it } from 'vitest';
import { rommConnecting, rommFlag, rommLive, rommPlatformRows, rommRailGroup, rommSelectionFallback } from './rail';
import { installed, platform, rommStatus } from './fixtures.test-util';

const servers = [
  platform({ id: 1, slug: 'snes', display_name: 'Super Nintendo Entertainment System', rom_count: 4 }),
  platform({ id: 2, slug: 'n64', name: 'Nintendo 64', display_name: 'Nintendo 64', rom_count: 2 }),
  platform({ id: 3, slug: 'gb', name: 'Game Boy', display_name: 'Game Boy', rom_count: 0 }),
  platform({ id: 4, slug: 'emulators', name: 'Emulators', display_name: 'Emulators', rom_count: 3 }),
];
const rows = [
  installed({ title: 'Chrono Trigger', rom_id: 42 }),
  installed({ title: 'Saturn Bomberman', platform: 'Sega Saturn', platform_slug: 'saturn', rom_id: null }),
  installed({ title: 'RetroArch', platform: 'Emulators', platform_slug: '' }),
  installed({ title: 'Witcher', platform: 'PC (Windows)', platform_slug: 'win', source: 'gog', rom_id: null }),
];

const keys = (g: ReturnType<typeof rommRailGroup>) => g?.rows.map((r) => r.key);

describe('rommRailGroup', () => {
  it('connected: Installed, Discover, then the platform rows by label', () => {
    const g = rommRailGroup(rommStatus(), servers, rows);
    expect(g?.heading).toBe('ROMM');
    expect(g?.flag).toBeNull();
    expect(keys(g)).toEqual(['romm:installed', 'romm:discover', 'platform:n64', 'platform:saturn', 'platform:snes']);
    // Installed counts RomM rows only, not the Emulators shelf or store rows.
    expect(g?.rows[0]).toMatchObject({ label: 'Installed', count: 2, testId: 'library-rail-romm-installed', countTestId: 'library-rail-count-romm-installed' });
    expect(g?.rows[1]).toMatchObject({ label: 'Discover', count: null, testId: 'library-rail-romm-discover' });
  });

  it('counts RomM games only on platform rows (decision 14)', () => {
    const g = rommRailGroup(rommStatus(), servers, rows);
    const snes = g?.rows.find((r) => r.key === 'platform:snes');
    const saturn = g?.rows.find((r) => r.key === 'platform:saturn');
    expect(snes).toMatchObject({ count: 4, label: 'Super Nintendo Entertainment System', testId: 'library-rail-platform-snes' });
    expect(snes?.platform?.id).toBe(1);
    expect(saturn).toMatchObject({ count: 0, label: 'Sega Saturn', platform: null });
  });

  it('connected while the platform list loads: Installed and Discover', () => {
    expect(keys(rommRailGroup(rommStatus(), null, rows))).toEqual(['romm:installed', 'romm:discover']);
  });

  it('connecting at startup counts as live: Installed and Discover, with a spinner flag', () => {
    const status = rommStatus({ connection: 'not_connected', busy: true });
    const g = rommRailGroup(status, null, []);
    expect(keys(g)).toEqual(['romm:installed', 'romm:discover']);
    expect(g?.flag).toEqual({ text: 'busy', title: 'Connecting to RomM' });
  });

  it.each([
    ['not_connected', { text: 'off', title: 'Not connected' }],
    ['unreachable', { text: '!', title: "Can't reach RomM" }],
    ['expired', { text: '!', title: 'RomM sign-in expired' }],
  ] as const)('%s: Installed only, with its flag', (connection, flag) => {
    const g = rommRailGroup(rommStatus({ connection }), servers, rows);
    expect(keys(g)).toEqual(['romm:installed']);
    expect(g?.flag).toEqual(flag);
  });

  it('no group when not connected and nothing RomM is installed (in_rail false)', () => {
    const status = rommStatus({ connection: 'not_connected', in_rail: false, server_url: '' });
    expect(rommRailGroup(status, null, [rows[2], rows[3]])).toBeNull();
    expect(rommRailGroup({ ...status, server_url: 'https://romm.home.lan' }, null, [])).toBeNull();
  });

  it('unreachable keeps the group even with nothing installed', () => {
    expect(keys(rommRailGroup(rommStatus({ connection: 'unreachable' }), null, []))).toEqual(['romm:installed']);
  });
});

describe('rommPlatformRows', () => {
  it('drops empty server platforms and the Emulators shelf, adds installed-only platforms', () => {
    expect(rommPlatformRows(servers, rows).map((r) => r.slug)).toEqual(['n64', 'saturn', 'snes']);
  });
});

describe('connecting and live', () => {
  it('connecting needs a stored server, a probe and not connected yet', () => {
    expect(rommConnecting(rommStatus({ connection: 'not_connected', busy: true }))).toBe(true);
    expect(rommConnecting(rommStatus({ connection: 'not_connected', busy: true, server_url: '' }))).toBe(false);
    expect(rommConnecting(rommStatus({ connection: 'unreachable', busy: true }))).toBe(false);
    expect(rommLive(rommStatus())).toBe(true);
    expect(rommLive(rommStatus({ connection: 'expired' }))).toBe(false);
  });
  it('a reconnect from Can\'t reach keeps the ! flag', () => {
    expect(rommFlag(rommStatus({ connection: 'unreachable', busy: true }))?.text).toBe('!');
  });
});

describe('rommSelectionFallback', () => {
  const connected = rommRailGroup(rommStatus(), servers, rows);
  const off = rommRailGroup(rommStatus({ connection: 'not_connected' }), null, rows);
  it('keeps a key that still exists, and non-RomM keys', () => {
    expect(rommSelectionFallback('platform:snes', connected)).toBe('platform:snes');
    expect(rommSelectionFallback('all', null)).toBe('all');
    expect(rommSelectionFallback('store:gog:installed', off)).toBe('store:gog:installed');
  });
  it('falls back to Installed when Discover or a platform disappears', () => {
    expect(rommSelectionFallback('romm:discover', off)).toBe('romm:installed');
    expect(rommSelectionFallback('platform:snes', off)).toBe('romm:installed');
  });
  it('falls back to All games when the group is gone', () => {
    expect(rommSelectionFallback('romm:installed', null)).toBe('all');
  });
});
