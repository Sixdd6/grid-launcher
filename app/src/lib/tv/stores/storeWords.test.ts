import { describe, expect, it } from 'vitest';
import { installedGame, row, status, storeGame } from '../../gamestores/fixtures.test-util';
import { mixedStoreGames, storeGameFromDrive } from './libraryStores';
import {
  conflictPrompt,
  disconnectWords,
  refusalTitle,
  storeRowPill,
  storeRowSub,
  toolRow,
  tvChips,
  tvCloudTab,
  tvCountLine,
  tvCoverTag,
  tvDriveQuick,
  tvFailureCard,
  tvHeaderActions,
  tvInspectionView,
  tvJobTag,
  tvLeftLines,
  tvPanel,
  tvQuick,
  tvStoreActions,
  tvStoreButtons,
  tvStoreRowA,
  tvTabStep,
  tvTabs,
} from './storeWords';
import type { InstalledGame } from '../../api';

const NOW = 1_800_000_000;

describe('TV store header (TVStores § Library › Header)', () => {
  it('counts installed games, and owned ones with the refresh time', () => {
    expect(tvCountLine(status(), 'installed', 3, NOW)).toBe('3 games');
    expect(tvCountLine(status({ refreshed_at: NOW - 300 }), 'available', 5, NOW)).toBe('5 games you own and haven’t installed · updated 5 min ago');
    expect(tvCountLine(status({ connection: 'expired' }), 'available', 0, NOW)).toBe('Games you own and haven’t installed');
  });

  it('chips: the account, a focusable Login expired, Not connected, then Cloud saves', () => {
    expect(tvChips(status()).map((c) => c.key)).toEqual(['account', 'cloud']);
    const expired = tvChips(status({ connection: 'expired' }));
    expect(expired[0]).toMatchObject({ text: 'Login expired · Sign in again', signIn: true, warn: true });
    expect(tvChips(status({ connection: 'not_connected' })).map((c) => c.text)).toEqual(['Not connected']);
  });

  it('Backup all N… and RT Select only in GOG › Installed; Refresh only on a listing Available', () => {
    expect(tvHeaderActions(status(), 'installed', 3)).toEqual({ backupAll: 3, installer: true, refresh: false, select: true });
    expect(tvHeaderActions(status(), 'available', 3)).toEqual({ backupAll: 0, installer: true, refresh: true, select: false });
    expect(tvHeaderActions(status({ owned: 'loading' }), 'available', 3).refresh).toBe(false);
  });

  it('a panel’s first button is the main one', () => {
    const p = tvPanel(status(), 'installed', []);
    expect(p?.actions.map((a) => [a.label, a.primary])).toEqual([
      ['Open Available', true],
      ['Install from installer…', false],
    ]);
    expect(tvPanel(status(), 'available', [])?.actions[0]).toMatchObject({ label: 'Refresh', primary: true });
  });
});

describe('TV store covers', () => {
  it('one tag: NOT SUPPORTED, else SAVE CONFLICT, else UPDATE (not while a job runs)', () => {
    expect(tvCoverTag(installedGame({ cloud: 'conflict', update_version: '2.2' }), null)?.text).toBe('SAVE CONFLICT');
    expect(tvCoverTag(installedGame({ update_version: '2.2' }), null)?.text).toBe('UPDATE');
    expect(tvCoverTag(installedGame({ update_version: '2.2' }), row({ source_id: 'gog:1001' }))).toBeNull();
    expect(tvCoverTag(storeGame({ not_supported: 'the EA app' }), null)?.text).toBe('NOT SUPPORTED');
  });

  it('the job tag and bar: Queued, or the percentage', () => {
    expect(tvJobTag(row({ status: 'queued' }))).toEqual({ text: 'Queued', pct: 0 });
    expect(tvJobTag(row())).toEqual({ text: '45%', pct: 45 });
  });

  it('X: Play, Install…, nothing while a job runs, signed out or not supported', () => {
    expect(tvQuick(installedGame(), status(), null)).toBe('Play');
    expect(tvQuick(storeGame(), status(), null)).toBe('Install…');
    expect(tvQuick(storeGame(), status({ connection: 'expired' }), null)).toBeNull();
    expect(tvQuick(storeGame(), status(), row())).toBeNull();
    expect(tvQuick(storeGame({ not_supported: 'x' }), status(), null)).toBeNull();
    expect(tvDriveQuick(false, null)).toBe('Install from drive');
    expect(tvDriveQuick(true, null)).toBe('Play');
  });
});

describe('TV store game page', () => {
  it('buttons in the prototype’s order for an installed GOG game with an update', () => {
    const b = tvStoreButtons({ game: installedGame({ update_version: '1.1' }), status: status(), job: null, fromDrive: false });
    expect(b.map((x) => x.label)).toEqual(['Play', 'Update to 1.1', 'Game settings', 'Verify and repair', 'Uninstall', 'Backup to drive…']);
  });

  it('a running job leads with Cancel … · 45%; a drive page with Install from drive', () => {
    expect(tvStoreButtons({ game: storeGame(), status: status(), job: row(), fromDrive: false })[0].label).toBe('Cancel install · 45%');
    expect(tvStoreButtons({ game: storeGame(), status: status(), job: null, fromDrive: true }).map((x) => x.label)).toEqual(['Install from drive', 'Remove from drive…']);
  });

  it('Install… is disabled with why when signed out', () => {
    const b = tvStoreButtons({ game: storeGame(), status: status({ connection: 'not_connected' }), job: null, fromDrive: false })[0];
    expect(b).toMatchObject({ label: 'Install…', disabled: true, why: 'Connect GOG in Settings › Stores to install.' });
  });

  it('left column: last played, how it runs, the cloud state, offline, on a drive', () => {
    const lines = tvLeftLines({
      game: installedGame({ origin: 'offline', cloud: 'conflict' }),
      nowSeconds: NOW,
      lastPlayedAt: NOW - 2 * 86400,
      compat: 'GE-Proton10-17',
      drive: null,
      onDrives: ['Couch kit'],
    });
    const text = lines.map((l) => l.segs.map((s) => s.text).join(''));
    expect(text).toEqual(['Last played 2 days ago', 'Runs natively · Linux version', 'Cloud save conflict. See Cloud saves.', 'Installed from an offline installer', 'On Couch kit']);
  });

  it('tabs: Cloud saves only for an installed game; LB / RB wrap', () => {
    expect(tvTabs(installedGame())).toEqual(['Overview', 'Media', 'Cloud saves']);
    expect(tvTabs(storeGame())).toEqual(['Overview', 'Media']);
    expect(tvTabStep(['Overview', 'Media'], 'Overview', -1)).toBe('Media');
  });

  it('the Cloud saves tab and the choice before launch', () => {
    const c = { state: 'conflict' as const, synced_at: 0, locations: [], conflict: { local_at: NOW - 60, local_files: 2, local_bytes: 2.1 * 1024 * 1024, cloud_at: NOW - 7200 } };
    const tab = tvCloudTab(installedGame({ cloud: 'conflict' }), c, NOW, false);
    expect(tab.kind).toBe('conflict');
    if (tab.kind === 'conflict') expect(tab.local.sub).toMatch(/2\.1 MB · uploads it to GOG$/);
    expect(tvCloudTab(installedGame({ cloud: 'off' }), null, NOW, false)).toMatchObject({ title: 'Cloud saves are off for this game', sub: 'Turn them on in Game settings' });
    expect(tvCloudTab(installedGame(), null, NOW, true)).toMatchObject({ kind: 'synced', title: 'Syncing…' });
    expect(conflictPrompt(installedGame({ title: 'Lanternfall' }), null, NOW).title).toBe('Which save should Lanternfall use?');
  });
});

describe('TV Settings › Stores', () => {
  it('the row says who is signed in and the status', () => {
    expect(storeRowSub(status())).toBe('Signed in as sam_plays');
    expect(storeRowSub(status({ tool: { ...status().tool, state: 'not_installed' } }))).toBe('Signed in as sam_plays · gogdl not installed');
    expect(storeRowPill(status({ connection: 'expired' }))).toEqual({ text: 'Login expired', tone: 'warn' });
  });

  it('the Store tool row and Disconnect’s words', () => {
    expect(toolRow(status())).toEqual({ info: true, value: 'gogdl 1.4.0 · installed' });
    expect(toolRow(status({ tool: { ...status().tool, state: 'failed', error: 'sha256 mismatch' } }))).toMatchObject({ label: 'Couldn’t install gogdl', value: 'Try again' });
    expect(disconnectWords(status({ installed_count: 3 })).body).toContain('The 3 installed games stay and still play');
  });
});

describe('TV offline installer', () => {
  it('a Windows installer runs silently only in TV mode', () => {
    const v = tvInspectionView(
      { path: '/d/setup_x.exe', kind: 'windows_exe', key: 'gog:1', title: 'Quarry Nine', cover: '', version: '1.4', bytes: 1, verdict: { kind: 'install' } },
      'GE-Proton10-17',
      true,
    );
    expect(v.kind === 'found' && v.how).toContain('TV mode never opens an installer’s own setup window');
    expect(refusalTitle('This is a macOS installer')).toBe('This is a macOS installer.');
  });
});

describe('TV Downloads store rows and installer cards', () => {
  it('A’s list: Cancel while live, Retry and Dismiss when failed, Open the game', () => {
    expect(tvStoreActions(row(), 'Copper Saints').map((a) => a.label)).toEqual(['Cancel', 'Open Copper Saints']);
    expect(tvStoreActions(row({ status: 'failed' }), null).map((a) => a.label)).toEqual(['Retry', 'Dismiss']);
    expect(tvStoreRowA(row({ status: 'failed' }))).toBe('Retry or dismiss');
  });

  it('the silent-only failure card offers Retry', () => {
    const e = row({ job: 'game', kind: 'base', title: 'Foxglove', status: 'failed', failure: { code: 'installer_needs_wizard', base_title: '', base_gog_id: '', kept: true } });
    expect(tvFailureCard(e)).toEqual({ text: 'Couldn’t install Foxglove silently', sub: 'This installer needs its setup window. Install it in desktop mode.', action: { kind: 'retry' } });
    const dlc = row({ job: 'game', kind: 'base', title: 'Low Tide Radio', status: 'failed', failure: { code: 'installer_dlc_base_missing', base_title: 'Pale Summit', base_gog_id: '', kept: false } });
    expect(tvFailureCard(dlc)?.action).toEqual({ kind: 'install_base', title: 'Pale Summit' });
  });
});

describe('store games in the Library’s own rows', () => {
  const rowOf = (store_id: string, last: number) => ({ source: 'gog', store_id, last_played_at: last, rom_id: null, platform: 'GOG' }) as unknown as InstalledGame;

  it('All games holds every installed one; Recent the ones played lately; Updates those with an update', () => {
    const a = installedGame({ title: 'B', store_id: '1' });
    const b = installedGame({ title: 'A', store_id: '2', update_version: '2.0' });
    const rows = [rowOf('1', NOW - 3600), rowOf('2', 0)];
    expect(mixedStoreGames([a, b], rows, 'all', NOW).map((g) => g.title)).toEqual(['A', 'B']);
    expect(mixedStoreGames([a, b], rows, 'recent', NOW).map((g) => g.title)).toEqual(['B']);
    expect(mixedStoreGames([a, b], rows, 'updates', NOW).map((g) => g.title)).toEqual(['A']);
  });

  it('a drive’s record stands in for a game GRID doesn’t own a record of', () => {
    const g = storeGameFromDrive({ key: 'gog:7', name: 'Paper Lanterns', platform: 'linux', layout: 'installed', version: '1.1', size: 10, cover: '', installed: false, added: '', summary: 's' });
    expect(g).toMatchObject({ key: 'gog:7', store_id: '7', title: 'Paper Lanterns', installed: false });
  });
});
