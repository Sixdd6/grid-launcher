// TV mode's Steam words (Phase 4): the TVStores prototype's Steam states —
// Install in Steam on X, the game page through the Steam client, the
// Playing page with no Stop, Downloads rows with no Cancel, Steam's row in
// Settings › Stores and its disconnect card.
import { describe, expect, it } from 'vitest';
import { row, steamGame, steamRow, steamStatus } from '../../gamestores/fixtures.test-util';
import { activeDownload } from '../topbar';
import {
  disconnectedCard,
  storeRowPill,
  storeRowSub,
  tvChips,
  tvCountLine,
  tvFacts,
  tvJobTag,
  tvLeftLines,
  tvManagedBox,
  tvPanel,
  tvPlayingLine,
  tvPlayingNote,
  tvQuick,
  tvStoreActions,
  tvStoreButtons,
  tvStoreLine,
  tvStoreListSub,
  tvStoreRowA,
  tvTabs,
} from './storeWords';

const now = 10_000_000;

describe('TV: Steam in the Library', () => {
  it('heads the view with the Steam account and "in your Steam libraries"', () => {
    expect(tvChips(steamStatus())).toEqual([{ key: 'account', text: 'Steam account: ', bold: 'SamPlays' }]);
    expect(tvCountLine(steamStatus(), 'installed', 3, now)).toBe('3 games · in your Steam libraries');
  });

  it('says its panels TV’s way: never desktop mode, no paste, no status code', () => {
    const noclient = tvPanel(steamStatus({}, { client_path: '' }), 'installed', [])!;
    expect(noclient.body).toBe('GRID installs, starts and uninstalls Steam games through the Steam client. Install Steam on this PC and sign in once, then press Check again.');
    expect(noclient.actions).toEqual([{ label: 'Check again', act: 'recheck', primary: true }]);
    expect(tvPanel(steamStatus({}, { key: 'none' }), 'available', [])!.actions.map((a) => a.label)).toEqual(['Open Settings › Stores']);
    const bad = tvPanel(steamStatus({}, { key: 'bad' }), 'available', [])!;
    expect(bad.body).toBe('The key may have been revoked. Make a new key, then enter it in Settings › Stores › Steam.');
    expect(tvPanel(steamStatus({}, { key: 'private' }), 'available', [])!.actions.map((a) => [a.label, a.primary])).toEqual([
      ['Refresh', true],
      ['Open Settings › Stores', false],
    ]);
  });

  it('X installs in Steam, plays an installed game, and does nothing while Steam installs', () => {
    expect(tvQuick(steamGame('Harbor of Bells'), steamStatus(), null)).toBe('Install in Steam');
    expect(tvQuick(steamGame('Moonlit Ferry'), steamStatus(), null)).toBe('Play');
    expect(tvQuick(steamGame('Quiet Comet'), steamStatus(), steamRow())).toBeNull();
  });
});

describe('TV: a Steam game’s page', () => {
  it('Play, Verify in Steam and Uninstall in Steam…; no Game settings', () => {
    expect(tvStoreButtons({ game: steamGame('Moonlit Ferry'), status: steamStatus(), job: null, fromDrive: false }).map((b) => b.label)).toEqual([
      'Play',
      'Verify in Steam',
      'Uninstall in Steam…',
    ]);
  });

  it('a disabled "Installing in Steam · 52%" whose A says why, and the managed box', () => {
    const job = steamRow();
    const [b] = tvStoreButtons({ game: steamGame('Quiet Comet'), status: steamStatus(), job, fromDrive: false });
    expect(b).toMatchObject({ label: 'Installing in Steam · 52%', disabled: true, why: 'Managed by Steam. Pause or cancel it in Steam.' });
    expect(tvManagedBox(job)).toEqual({ bold: 'Managed by Steam.', rest: ' Pause or cancel it in Steam.', pct: 52 });
    expect(tvManagedBox(row())).toBeNull();
  });

  it('Install in Steam for a game not installed', () => {
    expect(tvStoreButtons({ game: steamGame('Signal Lost'), status: steamStatus(), job: null, fromDrive: false })).toEqual([
      { act: 'install', label: 'Install in Steam', primary: true, icon: 'download' },
    ]);
  });

  it('the left column: last played, Launches through Steam, outside GRID, the no-backup line', () => {
    const lines = tvLeftLines({ game: steamGame('Old Brick Road'), nowSeconds: now, lastPlayedAt: 0, compat: '', drive: null, onDrives: [] });
    expect(lines.map((l) => l.segs.map((s) => s.text).join(''))).toEqual([
      'Last played never',
      'Launches through Steam',
      'Installed in Steam, outside GRID',
      'Steam games can’t be backed up here. Steam has its own Backup and restore.',
    ]);
  });

  it('facts with no Size for a game not installed; Overview and Media only', () => {
    expect(tvFacts(steamGame('Signal Lost')).map((f) => f.term)).toEqual(['Store', 'Developer', 'Release']);
    expect(tvFacts(steamGame('Moonlit Ferry')).find((f) => f.term === 'Size')?.value).toBe('9.1 GB');
    expect(tvTabs(steamGame('Moonlit Ferry'))).toEqual(['Overview', 'Media']);
  });

  it('Playing says Steam runs it', () => {
    expect(tvPlayingLine(steamGame('Moonlit Ferry'), 'GE-Proton10-17', '0:12')).toBe('Steam · running in Steam · 0:12');
    expect(tvPlayingNote(steamGame('Moonlit Ferry'))).toBe('Steam runs this game. Quit it from the game; GRID notices when it closes.');
  });
});

describe('TV: Steam in Downloads', () => {
  it('words the row and offers no Cancel or Retry', () => {
    const live = steamRow({ downloaded_bytes: 67 });
    expect(tvStoreLine(live, '')).toBe('Installing in Steam 67% · Managed by Steam');
    expect(tvStoreRowA(live)).toBe('Open');
    expect(tvStoreActions(live, 'Quiet Comet')).toEqual([{ act: 'open', label: 'Open Quiet Comet' }]);
    expect(tvStoreListSub(live)).toBe('Steam · Managed by Steam: pause or cancel it in Steam');
    const done = steamRow({ status: 'completed', detail: 'Installed by Steam' });
    expect(tvStoreLine(done, '')).toBe('Installed by Steam');
    expect(tvStoreActions(done, null)).toEqual([{ act: 'dismiss', label: 'Dismiss' }]);
    expect(tvStoreActions(steamRow({ status: 'cancelled' }), null)).toEqual([{ act: 'dismiss', label: 'Dismiss' }]);
  });

  it('the top bar shows a job GRID runs before Steam’s install', () => {
    expect(activeDownload([steamRow({ id: 1 }), row({ id: 2, title: 'Copper Saints' })])?.title).toBe('Copper Saints');
    expect(activeDownload([steamRow({ id: 1 })])?.title).toBe('Quiet Comet');
  });
});

describe('TV: waiting for Steam to start the download (2026-11-14)', () => {
  const waiting = (over: Parameters<typeof steamRow>[0] = {}) =>
    steamRow({ id: 5, source_id: 'steam:9100005', title: 'Harbor of Bells', phase: 'waiting_for_store', downloaded_bytes: 0, ...over });

  it('the row: the desktop’s line, A "Stop waiting", and its list', () => {
    expect(tvStoreLine(waiting(), '')).toBe('Waiting for Steam to start the download');
    expect(tvStoreRowA(waiting())).toBe('Stop waiting');
    expect(tvStoreListSub(waiting())).toBe(
      'Steam · Waiting for Steam to start the download. If you cancelled it in Steam, stop waiting. If Steam installs it later, GRID still adds it.',
    );
    expect(tvStoreActions(waiting(), 'Harbor of Bells')).toEqual([
      { act: 'stop_waiting', label: 'Stop waiting' },
      { act: 'open', label: 'Open Harbor of Bells' },
    ]);
  });

  it('ended: the backend’s line, A Dismiss, no Retry and no "Managed by Steam"', () => {
    for (const detail of ['Not installed · you stopped waiting for Steam', 'Not installed · Steam didn’t start the download in 5 minutes']) {
      const e = waiting({ status: 'cancelled', phase: 'none', detail });
      expect(tvStoreLine(e, '')).toBe(detail);
      expect(tvStoreRowA(e)).toBe('Dismiss');
      expect(tvStoreActions(e, 'Harbor of Bells')).toEqual([
        { act: 'dismiss', label: 'Dismiss' },
        { act: 'open', label: 'Open Harbor of Bells' },
      ]);
      expect(tvStoreListSub(e)).toBe('Steam');
    }
  });

  it('the game page: primary "Stop waiting for Steam" and the waiting box with no bar', () => {
    const job = waiting();
    expect(tvStoreButtons({ game: steamGame('Harbor of Bells'), status: steamStatus(), job, fromDrive: false })).toEqual([
      { act: 'stop_waiting', label: 'Stop waiting for Steam', primary: true },
    ]);
    expect(tvManagedBox(job)).toEqual({
      bold: 'Waiting for Steam to start the download.',
      rest: ' Steam’s install window is open. If you cancelled it there, stop waiting.',
      pct: null,
    });
  });

  it('the game page shows "Installing in Steam" once Steam has started (the prototype’s skind fix)', () => {
    const job = waiting({ phase: 'none', downloaded_bytes: 20 });
    const [b] = tvStoreButtons({ game: steamGame('Harbor of Bells'), status: steamStatus(), job, fromDrive: false });
    expect(b).toMatchObject({ label: 'Installing in Steam · 20%', disabled: true });
    expect(tvManagedBox(job)?.bold).toBe('Managed by Steam.');
  });

  it('the cover: "Waiting" over an empty bar, no X', () => {
    expect(tvJobTag(waiting({ downloaded_bytes: 40 }))).toEqual({ text: 'Waiting', pct: 0 });
    expect(tvJobTag(steamRow())).toEqual({ text: '52%', pct: 52 });
    expect(tvQuick(steamGame('Harbor of Bells'), steamStatus(), waiting())).toBeNull();
  });
});

describe('TV: Settings › Stores › Steam', () => {
  it('the row: SteamID64 and Connected / Installed games only / Not connected', () => {
    expect(storeRowSub(steamStatus())).toBe('SteamID64 76561198000000001');
    expect(storeRowPill(steamStatus())).toEqual({ text: 'Connected', tone: 'ok' });
    expect(storeRowPill(steamStatus({}, { key: 'none' }))).toEqual({ text: 'Installed games only', tone: 'warn' });
    expect(storeRowSub(steamStatus({ connection: 'not_connected' }))).toBe('Not set up');
    expect(storeRowPill(steamStatus({ connection: 'not_connected' }))).toEqual({ text: 'Not connected', tone: '' });
  });

  it('the disconnect card says the key was deleted', () => {
    expect(disconnectedCard('steam')).toEqual({ text: 'Steam disconnected', sub: 'Your key was deleted from the keyring' });
  });
});
