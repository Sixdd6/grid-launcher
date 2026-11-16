import { describe, expect, it } from 'vitest';
import type { StoreSource, StoreStatus } from '../../api';
import { rommStatus } from '../../romm/fixtures.test-util';
import { nextReady, parentOf, rommLine, storeOffLine, welcomeButton, welcomeRows } from './welcomePage';

const none = (_s: StoreSource): StoreStatus | null => null;

describe('the TV welcome (TVRommStorefront README § The welcome)', () => {
  it('step 1: Next once a folder is chosen; the browser opens beside it', () => {
    expect(nextReady('')).toBe(false);
    expect(nextReady('  ')).toBe(false);
    expect(nextReady('/home/sam/Games')).toBe(true);
    expect(parentOf('/home/sam/Games/GRID')).toBe('/home/sam/Games');
    expect(parentOf('/GRID')).toBe('/');
    expect(parentOf('GRID')).toBe('');
  });

  it('step 2: five rows in name order, RomM between GOG and Steam', () => {
    const rows = welcomeRows(rommStatus({ connection: 'not_connected', server_url: '', username: '' }), none, false);
    expect(rows.map((r) => r.id)).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam']);
    expect(rows.map((r) => r.name)).toEqual(['Amazon Games', 'Epic Games', 'GOG', 'RomM', 'Steam']);
    expect(rows.every((r) => !r.connected && r.a === 'Connect')).toBe(true);
    expect(rows[3].sub).toBe('Your own game server. GRID browses it and installs its games with an emulator.');
    expect(rows[0].sub).toBe(storeOffLine('amazon'));
    expect(storeOffLine('gog')).toBe("Sign in on GOG's own page. GRID never sees your password.");
    expect(storeOffLine('steam')).toBe('Installed Steam games show as soon as Steam is set up here.');
  });

  it('Skip for now until something is connected, then Continue', () => {
    const off = welcomeRows(rommStatus({ connection: 'not_connected' }), none, false);
    expect(welcomeButton(off)).toBe('Skip for now');
    const on = welcomeRows(rommStatus({ username: 'sam' }), none, false);
    expect(on[3]).toMatchObject({ connected: true, a: 'Change', sub: 'Connected as sam · romm.home.lan' });
    expect(welcomeButton(on)).toBe('Continue');
  });

  it('after a Python import, the RomM row says it is filled in', () => {
    expect(rommLine(rommStatus({ connection: 'not_connected', username: 'sam' }), true)).toBe('From your old GRID: sam · romm.home.lan. Sign in again.');
    expect(rommLine(rommStatus({ connection: 'not_connected', server_url: '' }), true)).toBe(
      'Your own game server. GRID browses it and installs its games with an emulator.',
    );
  });
});
