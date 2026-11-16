import { describe, expect, it } from 'vitest';
import { amazonStatus, epicStatus, status as gogStatus, steamStatus } from './gamestores/fixtures.test-util';
import { rommStatus } from './romm/fixtures.test-util';
import {
  FOOTER,
  folderMissing,
  folderToSave,
  initialFolder,
  initialOpenRow,
  loginSub,
  STEAM_ROW,
  welcomeFooter,
  welcomeRows,
  type WelcomeRow,
} from './welcome';

const neverConnected = rommStatus({ connection: 'not_connected', server_url: '', username: '', auth_kind: null, in_rail: false });
const off = { connection: 'not_connected' as const, username: '' };

describe('welcomeRows', () => {
  it('lists the storefronts by name: Amazon, Epic, GOG, RomM, Steam', () => {
    expect(welcomeRows(neverConnected, []).map((r) => r.id)).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam']);
  });

  it('keeps the order whatever order the stores arrive in', () => {
    const stores = [steamStatus(off), gogStatus(off), amazonStatus(off), epicStatus(off)];
    expect(welcomeRows(neverConnected, stores).map((r) => r.id)).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam']);
  });

  it('words each row for a new user: login window, RomM form, Steam form', () => {
    const rows = welcomeRows(neverConnected, []);
    expect(rows.map((r) => [r.name, r.kind, r.connected])).toEqual([
      ['Amazon Games', 'login', false],
      ['Epic Games', 'login', false],
      ['GOG', 'login', false],
      ['RomM', 'romm', false],
      ['Steam', 'steam', false],
    ]);
    expect(rows[0].sub).toBe('Sign in on Amazon Games’s own page. GRID never sees your password.');
    expect(rows[2].sub).toBe(loginSub('gog'));
    expect(rows[3].sub).toBe('Your own game server. GRID browses it and installs its games with an emulator.');
    expect(rows[4].sub).toBe(STEAM_ROW.sub);
  });

  it('shows who is signed in on a connected row', () => {
    const rows = welcomeRows(rommStatus({ username: 'player', server_url: 'https://romm.home.lan' }), [
      gogStatus({ connection: 'connected', username: 'sam_plays' }),
      steamStatus({ connection: 'connected', username: 'SamPlays' }, { steam_id: '76561198000000001' }),
    ]);
    const by = Object.fromEntries(rows.map((r) => [r.id, r]));
    expect(by.gog).toMatchObject({ connected: true, sub: 'Signed in as sam_plays' });
    expect(by.romm).toMatchObject({ connected: true, sub: 'Connected as player · romm.home.lan' });
    expect(by.steam).toMatchObject({ connected: true, sub: 'SteamID64 76561198000000001 · SamPlays' });
    expect(by.amazon.connected).toBe(false);
  });

  it('reads an expired store or an unreachable RomM as not connected', () => {
    const rows = welcomeRows(rommStatus({ connection: 'unreachable' }), [epicStatus({ connection: 'expired' })]);
    expect(rows.find((r) => r.id === 'epic')!.connected).toBe(false);
    expect(rows.find((r) => r.id === 'romm')!.connected).toBe(false);
  });
});

describe('the library folder', () => {
  it('starts on the stored folder, else the suggested one', () => {
    expect(initialFolder('', ' /home/me/Games/GRID ')).toBe('/home/me/Games/GRID');
    expect(initialFolder(' /data/RomM ', '/home/me/Games/GRID')).toBe('/data/RomM');
    expect(initialFolder('', '')).toBe('');
  });

  it('is the one required step', () => {
    expect(folderMissing('')).toBe(true);
    expect(folderMissing('   ')).toBe(true);
    expect(folderMissing('/games')).toBe(false);
  });

  it('Skip and Continue write the folder only when it changed', () => {
    expect(folderToSave(' /games ', '')).toBe('/games');
    expect(folderToSave('/games', '/games')).toBeNull();
    expect(folderToSave('/new', '/games')).toBe('/new');
    expect(folderToSave('  ', '/games')).toBeNull();
  });
});

describe('welcomeFooter', () => {
  const rows = (connected: string[]): WelcomeRow[] =>
    welcomeRows(neverConnected, []).map((r) => ({ ...r, connected: connected.includes(r.id) }));

  it('nothing connected: Skip for now is the way on, Continue is disabled', () => {
    expect(welcomeFooter({ folder: '/games', rows: rows([]), setupNeeded: false, busy: false })).toEqual({
      skipShown: true,
      skipEnabled: true,
      continueEnabled: false,
      help: FOOTER.nothingYet,
    });
  });

  it('an empty folder holds both buttons', () => {
    const f = welcomeFooter({ folder: ' ', rows: rows([]), setupNeeded: false, busy: false });
    expect(f.skipEnabled).toBe(false);
    expect(f.continueEnabled).toBe(false);
    expect(welcomeFooter({ folder: '', rows: rows(['gog']), setupNeeded: false, busy: false }).continueEnabled).toBe(false);
  });

  it('something connected: Continue alone, and GRID opens on the Library', () => {
    expect(welcomeFooter({ folder: '/games', rows: rows(['gog']), setupNeeded: true, busy: false })).toEqual({
      skipShown: false,
      skipEnabled: false,
      continueEnabled: true,
      help: FOOTER.opensLibrary,
    });
  });

  it('after a first RomM connect, Continue leads to Choose your systems', () => {
    expect(welcomeFooter({ folder: '/games', rows: rows(['romm']), setupNeeded: true, busy: false }).help).toBe(FOOTER.nextSystems);
    expect(welcomeFooter({ folder: '/games', rows: rows(['romm']), setupNeeded: false, busy: false }).help).toBe(FOOTER.opensLibrary);
  });

  it('does not run twice while finishing', () => {
    expect(welcomeFooter({ folder: '/games', rows: rows([]), setupNeeded: false, busy: true }).skipEnabled).toBe(false);
    expect(welcomeFooter({ folder: '/games', rows: rows(['steam']), setupNeeded: false, busy: true }).continueEnabled).toBe(false);
  });
});

describe('initialOpenRow', () => {
  it('opens RomM after a Python import, until it connects', () => {
    expect(initialOpenRow(true, false)).toBe('romm');
    expect(initialOpenRow(true, true)).toBeNull();
    expect(initialOpenRow(false, false)).toBeNull();
  });
});
