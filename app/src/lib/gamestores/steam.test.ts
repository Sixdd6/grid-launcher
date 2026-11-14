// Steam in the shared store words (Phase 4, milestone D1): the store with no
// tool, whose client installs, uninstalls and starts its games. Every Steam
// state of the GameStores parity inventory that a pure module decides.
import { describe, expect, it } from 'vitest';
import { actionFor, currentTransfer, footerLine } from '../downloads/format';
import { coverPrimary, coverProgress, coverProgressTone, coverTags, showCoverCloud } from './cover';
import { managedBox, storeButtons, storeDetailChips, storeFacts, storeHeaderLine, storeLines, storeTabs } from './details';
import { entrySource, storeEntryLine, storeKindLabel, storeStripLine } from './downloads';
import { liveJobFor } from './jobs';
import { ENABLED_STORES, isEnabledStore, managedByClient, storeCaps, storesLede } from './marks';
import { disconnectedNotice, steamInstallNotice, steamRunningNotice, steamStillPrivateNotice, steamUninstallNotice, steamVerifyNotice } from './notices';
import { availBlock, storeFlag, storeRailGroups, storeShown } from './rail';
import {
  clientPlaceIsPath,
  initialManual,
  initialPick,
  keySaved,
  pickedSteamId,
  saveLabel,
  steamClientMissing,
  steamClientPlace,
  steamIdLine,
  steamPill,
  steamRowSub,
  testCardTv,
  testLine,
} from './steam';
import { storeChips, storeCountLine, storeHeaderActions, storePanel } from './view';
import { row, status, steamGame, steamRow, steamStatus } from './fixtures.test-util';
import { FAKE_STEAM_GUEST_ID, FAKE_STEAM_ID, FAKE_STEAM_KEY, FakeStores } from './fakeApi';

const NOW = 1_800_000_000;

describe('Steam is a store this build shows', () => {
  it('is enabled, last in the rail, with no tool and its client managing everything', () => {
    expect(ENABLED_STORES).toEqual(['amazon', 'epic', 'gog', 'steam']);
    expect(isEnabledStore('steam')).toBe(true);
    expect(managedByClient('steam')).toBe(true);
    expect(managedByClient('gog') || managedByClient('epic') || managedByClient('amazon')).toBe(false);
    expect(storeCaps('steam').hasTool).toBe(false);
  });

  it('names Steam in the Stores lede', () => {
    expect(storesLede()).toMatch(/GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; .* Steam games install and start through the Steam client\.$/);
  });
});

describe('the rail group', () => {
  it('shows the group with Installed and Available counts, last', () => {
    const groups = storeRailGroups([status(), steamStatus({ installed_count: 3, owned_count: 7 })]);
    expect(groups.map((g) => g.heading)).toEqual(['GOG', 'STEAM']);
    const steam = groups[1];
    expect(steam.flag).toBeNull();
    expect(steam.rows.map((r) => r.count)).toEqual([
      { kind: 'number', n: 3 },
      { kind: 'number', n: 4 },
    ]);
  });

  it('flags the heading and the Available count for a missing, rejected or private key', () => {
    for (const key of ['none', 'bad', 'private'] as const) {
      const s = steamStatus({}, { key });
      expect(availBlock(s)).toBe(key);
      expect(storeFlag(s)).toEqual({ text: '!', title: 'Needs attention' });
      expect(storeRailGroups([s])[0].rows[1].count).toEqual({ kind: 'attention' });
    }
  });

  it('flags the heading and the Installed count when the Steam client is missing', () => {
    const s = steamStatus({}, { client_path: '' });
    expect(steamClientMissing(s)).toBe(true);
    expect(storeFlag(s)).toEqual({ text: '!', title: 'Needs attention' });
    expect(storeRailGroups([s])[0].rows[0].count).toEqual({ kind: 'attention' });
    expect(storeRailGroups([s])[0].rows[1].count).toEqual({ kind: 'number', n: 4 });
  });

  it('spins while the owned list loads, and ignores the empty tool', () => {
    const s = steamStatus({ owned: 'loading' });
    expect(s.tool.state).toBe('not_installed');
    expect(availBlock(s)).toBe('loading');
    expect(storeRailGroups([s])[0].rows[1].count).toEqual({ kind: 'loading' });
  });

  it('has no group when not connected, even with games still installed in Steam', () => {
    const off = steamStatus({ connection: 'not_connected', in_rail: false, installed_count: 3 });
    expect(storeShown(off)).toBe(false);
    expect(storeRailGroups([off])).toEqual([]);
  });
});

describe('the header', () => {
  const inst = [steamGame('Moonlit Ferry'), steamGame('Deep Field Radio'), steamGame('Old Brick Road')];

  it('counts Installed with their size, in your Steam libraries', () => {
    expect(storeCountLine(steamStatus(), 'installed', inst, NOW)).toBe('3 games · 27.1 GB · in your Steam libraries');
  });

  it('counts Available with no size, or the plain line while blocked', () => {
    const avail = [steamGame('Quiet Comet'), steamGame('Harbor of Bells')];
    expect(storeCountLine(steamStatus({ refreshed_at: NOW - 300 }), 'available', avail, NOW)).toBe('2 games you own and haven’t installed · updated 5 min ago');
    expect(storeCountLine(steamStatus({}, { key: 'none' }), 'available', [], NOW)).toBe('Games you own and haven’t installed');
  });

  it('chips the Steam account, with no tool or cloud chip', () => {
    expect(storeChips(steamStatus()).map((c) => [c.text, c.act])).toEqual([['Steam account: SamPlays', 'settings']]);
  });

  it('offers Refresh on Available when it lists games, and no Select, Backup all or installer', () => {
    expect(storeHeaderActions(steamStatus(), 'available', 3)).toEqual({ backupAll: 0, installer: false, refresh: true, select: false });
    expect(storeHeaderActions(steamStatus(), 'installed', 3)).toEqual({ backupAll: 0, installer: false, refresh: false, select: false });
    expect(storeHeaderActions(steamStatus({}, { key: 'private' }), 'available', 3).refresh).toBe(false);
  });
});

describe('the panels in place of the grid', () => {
  it('says nothing is installed, with Steam’s own games too', () => {
    const p = storePanel(steamStatus(), 'installed', [])!;
    expect([p.title, p.body]).toEqual(['No Steam games installed yet', 'Install one from Available. Games you install in Steam itself show here too.']);
    expect(p.actions.map((a) => a.label)).toEqual(['Open Available']);
  });

  it('says Steam isn’t installed in Installed, with Check again', () => {
    const p = storePanel(steamStatus({}, { client_path: '' }), 'installed', [steamGame('Moonlit Ferry')])!;
    expect(p.title).toBe('Steam isn’t installed');
    expect(p.body).toMatch(/^GRID installs, starts and uninstalls Steam games through the Steam client\./);
    expect(p.actions).toEqual([{ label: 'Check again', act: 'recheck', primary: false }]);
  });

  it('asks for the key when none is saved', () => {
    const p = storePanel(steamStatus({}, { key: 'none' }), 'available', [])!;
    expect(p.title).toBe('Steam needs your Web API key to list the games you haven’t installed');
    expect(p.actions).toEqual([{ label: 'Add your key in Settings…', act: 'open_settings', primary: true }]);
  });

  it('says the key was rejected, with no status code', () => {
    const p = storePanel(steamStatus({}, { key: 'bad' }), 'available', [])!;
    expect([p.tone, p.title]).toEqual(['err', 'Steam rejected your Web API key']);
    expect(p.body).not.toMatch(/40[13]|Forbidden/);
    expect(p.actions.map((a) => a.act)).toEqual(['open_settings']);
  });

  it('gives the privacy path for private game details', () => {
    const p = storePanel(steamStatus({}, { key: 'private' }), 'available', [])!;
    expect([p.tone, p.title]).toEqual(['warn', 'Your Steam game details are private']);
    expect(p.actions.map((a) => [a.label, a.primary])).toEqual([
      ['Refresh', true],
      ['Open Settings › Stores', false],
    ]);
  });

  it('shows loading, everything installed and not connected as the other stores do', () => {
    expect(storePanel(steamStatus({ owned: 'loading' }), 'available', [])!.title).toBe('Loading your Steam games…');
    expect(storePanel(steamStatus(), 'available', [])!.title).toBe('Every Steam game you own is installed');
    expect(storePanel(steamStatus({ connection: 'not_connected' }), 'available', [])!.title).toBe('Steam isn’t connected');
  });
});

describe('covers', () => {
  it('Install in Steam on an Available cover, Play on an installed one, no cloud button', () => {
    expect(coverPrimary(steamGame('Quiet Comet'), steamStatus(), null)).toEqual({ label: 'Install in Steam', act: 'install' });
    expect(coverPrimary(steamGame('Moonlit Ferry'), steamStatus(), null)).toEqual({ label: 'Play', act: 'play' });
    expect(showCoverCloud(steamGame('Moonlit Ferry'))).toBe(false);
  });

  it('shows Steam’s install with its percentage, a teal bar and a disabled Installing in Steam', () => {
    const job = steamRow();
    expect(liveJobFor([job], 'steam:9100004')).toBe(job);
    expect(coverTags(steamGame('Quiet Comet'), job)).toEqual([{ text: '52%', tone: 'queue' }]);
    expect(coverProgress(job)).toBe(52);
    expect(coverProgressTone(job)).toBe('managed');
    expect(coverProgressTone(row())).toBe('primary');
    expect(coverPrimary(steamGame('Quiet Comet'), steamStatus(), job)).toEqual({ label: 'Installing in Steam', act: 'details', disabled: true, title: 'Managed by Steam' });
  });
});

describe('Details', () => {
  const ctx = (game = steamGame('Moonlit Ferry'), over: { job?: ReturnType<typeof steamRow> | null; playing?: boolean } = {}) => ({
    game,
    status: steamStatus(),
    job: over.job ?? null,
    playing: over.playing ?? false,
    confirm: null,
  });

  it('Play, Verify in Steam and Uninstall in Steam…, with no Game Settings', () => {
    expect(storeButtons(ctx()).map((b) => b.label)).toEqual(['Play', 'Verify in Steam', 'Uninstall in Steam…']);
    expect(storeButtons(ctx()).some((b) => b.destructive)).toBe(false);
  });

  it('Running in Steam while it plays', () => {
    expect(storeButtons(ctx(undefined, { playing: true }))[0]).toEqual({ label: 'Running in Steam', act: 'stop', primary: true });
  });

  it('Install in Steam for a game not installed', () => {
    expect(storeButtons(ctx(steamGame('Harbor of Bells'))).map((b) => b.label)).toEqual(['Install in Steam']);
  });

  it('the managed box and a disabled Cancel while Steam installs', () => {
    const job = steamRow();
    expect(storeButtons(ctx(steamGame('Quiet Comet'), { job }))).toEqual([{ label: 'Cancel', act: 'cancel', primary: false, disabled: true, title: 'Managed by Steam' }]);
    expect(managedBox(job)).toEqual({ title: 'Installing in Steam · 52%', pct: 52, note: 'Managed by Steam. Pause or cancel it in Steam.' });
    expect(managedBox(row())).toBeNull();
  });

  it('lines: last played, Launches through Steam, outside GRID, the no-backup line', () => {
    const lines = storeLines(ctx(steamGame('Deep Field Radio')), { nowSeconds: NOW, lastPlayedAt: NOW - 7 * 86400, compat: 'GE-Proton10-17', onDrives: [], syncing: false });
    expect(lines.map((l) => l.text)).toEqual([
      'Last played 1 week ago',
      'Launches through Steam',
      'Installed in Steam, outside GRID',
      'Steam games can’t be backed up here. Steam has its own Backup and restore.',
    ]);
    const mine = storeLines(ctx(), { nowSeconds: NOW, lastPlayedAt: 0, compat: '', onDrives: [], syncing: false });
    expect(mine.map((l) => l.text)).not.toContain('Installed in Steam, outside GRID');
  });

  it('a game not installed has no "Connect … to install" line', () => {
    const lines = storeLines({ ...ctx(steamGame('Harbor of Bells')), status: steamStatus({ connection: 'not_connected' }) }, { nowSeconds: NOW, lastPlayedAt: 0, compat: '', onDrives: [], syncing: false });
    expect(lines.map((l) => l.kind)).not.toContain('why');
  });

  it('chips: the store and Playing only', () => {
    const g = steamGame('Moonlit Ferry', { installed_platform: 'linux', version: '123' });
    expect(storeDetailChips(g, true).map((c) => c.text)).toEqual(['Steam', 'Playing']);
  });

  it('facts: no Folder, no Genres, and no Size for a game not installed', () => {
    expect(storeFacts(steamGame('Moonlit Ferry', { genres: ['Adventure'] })).map((f) => f.term)).toEqual(['Store', 'Developer', 'Released', 'Size']);
    expect(storeFacts(steamGame('Moonlit Ferry')).find((f) => f.term === 'Size')!.value).toBe('9.1 GB on disk');
    expect(storeFacts(steamGame('Harbor of Bells')).map((f) => f.term)).toEqual(['Store', 'Developer', 'Released']);
    expect(storeHeaderLine(steamGame('Moonlit Ferry', { genres: ['Adventure'] }))).toBe('Steam · 2024 · Tideglass');
    expect(storeTabs(steamGame('Moonlit Ferry'))).toEqual(['overview', 'media']);
  });
});

describe('Downloads', () => {
  it('words Steam’s row and ends it Installed by Steam', () => {
    expect(storeEntryLine(steamRow())).toBe('Installing in Steam 52% · Managed by Steam');
    expect(storeEntryLine(steamRow({ status: 'completed', detail: 'Installed by Steam' }))).toBe('Installed by Steam');
    expect(storeEntryLine(steamRow({ status: 'completed' }))).toBe('Installed by Steam');
    expect(storeEntryLine(steamRow({ status: 'cancelled' }))).toBe('Cancelled');
    expect(storeKindLabel('store_external')).toBe('');
    expect(entrySource(steamRow())).toBe('steam');
  });

  it('offers a disabled Cancel while live and Dismiss after, never Retry', () => {
    expect(actionFor('downloading', 'store_external')).toBe('managed');
    expect(actionFor('completed', 'store_external')).toBe('dismiss');
    expect(actionFor('cancelled', 'store_external')).toBe('dismiss');
    expect(actionFor('failed', 'store_external')).toBe('dismiss');
  });

  it('the strip prefers a job GRID runs, and shows Steam’s without "Managed by Steam" when alone', () => {
    const gog = row({ id: 2 });
    const steam = steamRow({ id: 3 });
    expect(currentTransfer([steam, gog])).toBe(gog);
    expect(currentTransfer([steam])).toBe(steam);
    expect(footerLine([steam])).toBe('Quiet Comet · Steam · Installing in Steam 52%');
    expect(storeStripLine(steam)).toBe('Quiet Comet · Steam · Installing in Steam 52%');
  });
});

describe('cards', () => {
  it('words the cards for what the user pressed', () => {
    expect(steamInstallNotice('Quiet Comet').text).toBe('Steam opened to install Quiet Comet. Choose the library folder there; GRID shows the progress.');
    expect(steamVerifyNotice('Moonlit Ferry').text).toBe('Steam is verifying Moonlit Ferry’s files. Watch it in Steam.');
    expect(steamUninstallNotice('Moonlit Ferry').text).toBe('Steam opened its uninstall window for Moonlit Ferry. It leaves the Library once Steam removes it.');
    expect(steamRunningNotice('Moonlit Ferry').text).toBe('Moonlit Ferry is running in Steam. Quit it from the game.');
    expect(steamStillPrivateNotice()).toMatchObject({ error: true, text: 'Steam still lists no games: the profile’s game details are private' });
    expect(disconnectedNotice('steam').text).toBe('Steam disconnected. Your key was deleted from the keyring.');
  });
});

describe('Settings › Stores › Steam', () => {
  it('pills Connected, Installed games only and Not connected', () => {
    expect(steamPill(steamStatus())).toEqual({ text: 'Connected', tone: 'ok' });
    expect(steamPill(steamStatus({}, { key: 'none' }))).toEqual({ text: 'Installed games only', tone: 'warn' });
    expect(steamPill(steamStatus({}, { key: 'private' }))).toEqual({ text: 'Installed games only', tone: 'warn' });
    expect(steamPill(steamStatus({ connection: 'not_connected' }))).toEqual({ text: 'Not connected', tone: 'off' });
  });

  it('heads the card with the SteamID64 and persona', () => {
    expect(steamIdLine(steamStatus())).toBe('SteamID64 76561198000000001 · SamPlays');
    expect(steamIdLine(steamStatus({ connection: 'not_connected' }))).toBe('Not set up');
    expect(steamRowSub(steamStatus())).toBe('SteamID64 76561198000000001');
  });

  it('shows the client’s path, a Flatpak’s package, or nothing when Steam is missing', () => {
    expect(steamClientPlace(steamStatus())).toBe('/usr/bin/steam');
    expect(clientPlaceIsPath('/usr/bin/steam')).toBe(true);
    expect(steamClientPlace(steamStatus({}, { client_path: 'Flatpak (com.valvesoftware.Steam)' }))).toBe('Flatpak (com.valvesoftware.Steam)');
    expect(clientPlaceIsPath('Flatpak (com.valvesoftware.Steam)')).toBe(false);
    expect(steamClientPlace(steamStatus({}, { client_path: '' }))).toBeNull();
  });

  it('shows a working saved key as dots only, and the field otherwise', () => {
    expect(keySaved(steamStatus())).toBe(true);
    expect(keySaved(steamStatus({}, { key: 'bad' }))).toBe(false);
    expect(keySaved(steamStatus({}, { key: 'none' }))).toBe(false);
    expect(keySaved(steamStatus({ connection: 'not_connected' }))).toBe(false);
  });

  it('starts on the saved account, else the most recent local one, else manual entry', () => {
    expect(initialPick(steamStatus())).toBe('76561198000000001');
    expect(initialPick(steamStatus({}, { steam_id: '' }))).toBe('76561198000000001');
    expect(initialPick(steamStatus({}, { steam_id: '76561198000000077' }))).toBe('manual');
    expect(initialManual(steamStatus({}, { steam_id: '76561198000000077' }))).toBe('76561198000000077');
    expect(initialPick(steamStatus({}, { steam_id: '', accounts: [] }))).toBe('manual');
    expect(pickedSteamId('manual', ' 76561198000000077 ')).toBe('76561198000000077');
    expect(saveLabel(steamStatus({ connection: 'not_connected' }))).toBe('Connect Steam');
    expect(saveLabel(steamStatus())).toBe('Save');
  });

  it('words the five Test results, the rejected key with no status code', () => {
    expect(testLine({ kind: 'ok', owned_count: 4 })).toEqual({ ok: true, text: 'The key works · 4 games owned' });
    expect(testLine({ kind: 'bad_key', status: 403 }).text).toBe('Steam rejected the key. Check that you copied all 32 characters, or make a new key.');
    expect(testLine({ kind: 'private' }).help).toBe('In Steam: your profile › Edit Profile › Privacy Settings › set Game details to Public. Then test again.');
    expect(testLine({ kind: 'bad_steam_id' }).text).toBe('That isn’t a SteamID64. It has 17 digits and starts with 7656.');
    expect(testLine({ kind: 'network' }).text).toBe('Couldn’t reach api.steampowered.com. Check your connection and try again.');
    expect(testCardTv({ kind: 'bad_key', status: 401 })).toEqual({ text: 'Steam rejected the key', sub: 'Check that you entered all 32 characters, or make a new key.', error: true });
    expect(testCardTv({ kind: 'ok', owned_count: 4 })).toEqual({ text: 'The key works', sub: '4 games owned', error: false });
  });
});

describe('the fake Steam backend', () => {
  it('tests and saves with the fixture key only, and never puts the key in a status', async () => {
    const events: [string, unknown][] = [];
    const fake = new FakeStores((e, p) => events.push([e, p]));
    fake.steamDelayMs = 0;
    expect(await fake.handle('steam_test', { steamId: '123', key: FAKE_STEAM_KEY })).toEqual({ kind: 'bad_steam_id' });
    expect(await fake.handle('steam_test', { steamId: FAKE_STEAM_ID, key: 'wrong' })).toEqual({ kind: 'bad_key', status: 401 });
    expect(await fake.handle('steam_test', { steamId: FAKE_STEAM_GUEST_ID, key: FAKE_STEAM_KEY })).toEqual({ kind: 'private' });
    fake.scenario('notConnected', 'steam');
    expect(await fake.handle('steam_save', { steamId: FAKE_STEAM_ID, key: FAKE_STEAM_KEY })).toEqual({ kind: 'ok', owned_count: 7 });
    expect(fake.library('steam').status.connection).toBe('connected');
    expect(JSON.stringify(events)).not.toContain(FAKE_STEAM_KEY);
    expect(JSON.stringify(fake.allStatuses())).not.toContain(FAKE_STEAM_KEY);
  });

  it('opens Steam to install: a Steam row that takes no slot', () => {
    const fake = new FakeStores();
    fake.handle('steam_open', { key: 'steam:9100005', action: 'install' });
    expect(fake.entries.find((e) => e.source_id === 'steam:9100005')).toMatchObject({ kind: 'store_external', status: 'downloading', platform: 'Steam' });
    fake.scenario('noclient', 'steam');
    expect(() => fake.handle('steam_open', { key: 'steam:9100006', action: 'install' })).toThrow('Steam isn’t installed. Install Steam, then try again.');
  });
});
