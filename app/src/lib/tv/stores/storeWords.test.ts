import { describe, expect, it } from 'vitest';
import { installedGame, row, status, storeGame } from '../../gamestores/fixtures.test-util';
import { mixedStoreGames, storeGameFromDrive } from './libraryStores';
import {
  conflictPrompt,
  deleteInstallerWords,
  deletesInstaller,
  disconnectedCard,
  disconnectWords,
  loginHost,
  notConnectedNote,
  tvStoreListSub,
  tvStoreLine,
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
import type { InstalledGame, StoreCloudState, StoreGame, StoreStatus } from '../../api';

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
    const c = { state: 'conflict' as const, synced_at: 0, locations: [], conflict: { local_at: NOW - 60, local_files: 2, local_bytes: 2.1 * 1024 * 1024, cloud_at: NOW - 7200, cloud_files: 3, cloud_bytes: 2.0 * 1024 * 1024 } };
    const tab = tvCloudTab(installedGame({ cloud: 'conflict' }), c, NOW, false);
    expect(tab.kind).toBe('conflict');
    if (tab.kind === 'conflict') {
      expect(tab.local.sub).toMatch(/2\.1 MB · uploads it to GOG$/);
      expect(tab.cloud.sub).toMatch(/ on another computer · 2\.0 MB · replaces the save here$/);
    }
    const unsized = tvCloudTab(installedGame({ cloud: 'conflict' }), { ...c, conflict: { ...c.conflict, cloud_bytes: null } }, NOW, false);
    if (unsized.kind === 'conflict') expect(unsized.cloud.sub).toMatch(/ on another computer · replaces the save here$/);
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
      { path: '/d/setup_x.exe', kind: 'windows_exe', key: 'gog:1', title: 'Quarry Nine', cover: '', version: '1.4', bytes: 1, verdict: { kind: 'install' }, free_bytes: null },
      'GE-Proton10-17',
      true,
    );
    const how = v.kind === 'found' ? v.how : '';
    expect(how).toContain('GRID tries once more with gentler settings');
    expect(how).toContain('so you can retry or delete it');
    expect(how).not.toMatch(/desktop/i);
    expect(refusalTitle('This is a macOS installer')).toBe('This is a macOS installer.');
  });
});

describe('TV Downloads store rows and installer cards', () => {
  it('A’s list: Cancel while live, Retry and Dismiss when failed, Open the game', () => {
    expect(tvStoreActions(row(), 'Copper Saints').map((a) => a.label)).toEqual(['Cancel', 'Open Copper Saints']);
    expect(tvStoreActions(row({ status: 'failed' }), null).map((a) => a.label)).toEqual(['Retry', 'Dismiss']);
    expect(tvStoreRowA(row({ status: 'failed' }))).toBe('Retry or dismiss');
  });

  it('Delete installer follows Retry on a failed silent install, asked first', () => {
    const error = "Couldn't run the installer silently: there isn't enough disk space. The installer was kept.";
    const offline = row({ kind: 'store_installer', title: 'Quarry Nine', status: 'failed', error });
    expect(tvStoreActions(offline, null).map((a) => a.label)).toEqual(['Retry', 'Delete installer', 'Dismiss']);
    expect(deletesInstaller(row({ job: 'game', kind: 'base', status: 'failed', error }))).toBe(true);
    // A drive restore's installer stays on the drive; other failures have none.
    expect(deletesInstaller(row({ kind: 'store_restore', status: 'failed', error }))).toBe(false);
    expect(deletesInstaller(row({ kind: 'store_installer', status: 'failed', error: 'boom' }))).toBe(false);
    expect(deleteInstallerWords(offline)).toEqual({
      title: 'Delete Quarry Nine’s installer?',
      body: 'The installer file and its parts are deleted from this computer. This can’t be undone.',
      ok: 'Delete installer',
    });
  });

  it('a failed silent install’s card gives the reason and offers Retry, never desktop mode', () => {
    const disk = row({
      job: 'game',
      kind: 'base',
      title: 'Foxglove',
      status: 'failed',
      error: "Couldn't run the installer silently: there isn't enough disk space. The installer was kept.",
    });
    expect(tvFailureCard(disk)).toEqual({ text: 'Couldn’t install Foxglove silently', sub: 'There isn’t enough disk space.', action: { kind: 'retry' } });
    const reasons: [string, string][] = [
      ['a file it needed was in use or access was denied', 'A file it needed was in use or access was denied.'],
      ["the installer's files are missing or damaged", 'The installer’s files are missing or damaged.'],
      ['the installer cancelled itself', 'The installer cancelled itself.'],
      ['the installer stopped without saying why', 'The installer stopped without saying why.'],
    ];
    for (const [words, sub] of reasons) {
      const e = row({ kind: 'store_installer', title: 'Quarry Nine', status: 'failed', error: `Couldn't run the installer silently: ${words}. The installer was kept.` });
      const c = tvFailureCard(e);
      expect(c?.sub).toBe(sub);
      expect(`${c?.text} ${c?.sub} ${tvStoreLine(e, 'GE-Proton10-17', '')}`).not.toMatch(/desktop/i);
    }
    // The queue's older code reads as the unknown reason.
    const old = row({ job: 'game', kind: 'base', title: 'Foxglove', status: 'failed', failure: { code: 'installer_needs_wizard', base_title: '', base_gog_id: '', kept: true } });
    expect(tvFailureCard(old)).toEqual({ text: 'Couldn’t install Foxglove silently', sub: 'The installer stopped without saying why.', action: { kind: 'retry' } });
    expect(tvStoreLine(old, '', '')).not.toMatch(/desktop/i);
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

describe('Epic Games in TV mode (docs/stores-phase2-plan.md package E)', () => {
  const epicStatus = (over: Partial<StoreStatus> = {}) =>
    status({ source: 'epic', username: 'SamPlays', ...over, tool: { ...status().tool, name: 'legendary', version: '0.21.1', license: 'GPL-3.0', size_bytes: 3_093_622, ...(over.tool ?? {}) } });
  const epicGame = (over: Partial<StoreGame> = {}) =>
    storeGame({ source: 'epic', key: 'epic:Glimmer', store_id: 'Glimmer', title: 'Glimmer Deep', platforms: ['windows'], origin: null, ...over });
  const epicInstalled = (over: Partial<StoreGame> = {}) =>
    installedGame({ source: 'epic', key: 'epic:Starlight', store_id: 'Starlight', title: 'Starlight Courier', platforms: ['windows'], installed_platform: 'windows', origin: 'legendary', ...over });

  it('the header has no Backup all, installer button or Select; Refresh on Available', () => {
    expect(tvHeaderActions(epicStatus(), 'installed', 2)).toEqual({ backupAll: 0, installer: false, refresh: false, select: false });
    expect(tvHeaderActions(epicStatus(), 'available', 2)).toEqual({ backupAll: 0, installer: false, refresh: true, select: false });
    expect(tvChips(epicStatus()).map((c) => `${c.text}${c.bold ?? ''}`)).toEqual(['Signed in as SamPlays', 'Cloud saves: on']);
  });

  it('nothing installed offers Open Available only (no installer)', () => {
    expect(tvPanel(epicStatus(), 'installed', [])?.actions.map((a) => a.label)).toEqual(['Open Available']);
  });

  it('a game that needs another launcher: NOT SUPPORTED, no X, a disabled Not supported, and why', () => {
    const g = epicGame({ not_supported: 'the EA app' });
    expect(tvCoverTag(g, null)).toEqual({ text: 'NOT SUPPORTED', cls: 'sns' });
    expect(tvQuick(g, epicStatus(), null)).toBeNull();
    expect(tvStoreButtons({ game: g, status: epicStatus(), job: null, fromDrive: false })).toEqual([
      expect.objectContaining({ label: 'Not supported', disabled: true, why: 'Needs the EA app. GRID can’t install games that start through another company’s launcher.' }),
    ]);
    const lines = tvLeftLines({ game: g, nowSeconds: NOW, lastPlayedAt: 0, compat: 'GE-Proton9-20', drive: null, onDrives: [] });
    expect(lines.map((l) => l.segs.map((s) => s.text).join(''))).toEqual([
      'Needs the EA app. GRID can’t install games that start through another company’s launcher.',
      'Windows only · runs through GE-Proton9-20',
      'Epic Games games can’t be backed up: legendary installs only from Epic’s servers.',
    ]);
  });

  it('an installed game has no Backup to drive…, runs through its tool, and has Cloud saves', () => {
    const g = epicInstalled({ compat_tool: 'GE-Proton9-20', cloud: 'synced', last_sync: NOW - 4 * 86400 });
    const acts = tvStoreButtons({ game: g, status: epicStatus(), job: null, fromDrive: false }).map((b) => b.label);
    expect(acts).toEqual(['Play', 'Game settings', 'Verify and repair', 'Uninstall']);
    const lines = tvLeftLines({ game: g, nowSeconds: NOW, lastPlayedAt: NOW - 4 * 86400, compat: 'GE-Proton9-20', drive: null, onDrives: ['Couch kit'] });
    expect(lines.map((l) => l.key)).toEqual(['played', 'runs', 'cloud', 'nobk']);
    expect(lines[1].segs.map((s) => s.text).join('')).toBe('Runs through GE-Proton9-20 · Windows version');
    expect(tvTabs(g)).toEqual(['Overview', 'Media', 'Cloud saves']);
    expect(tvCloudTab(g, null, NOW, false)).toMatchObject({ kind: 'synced', sub: 'GRID syncs with Epic Games before the game starts and after it closes' });
    expect(tvCloudTab(epicInstalled({ cloud: 'none' }), null, NOW, false)).toEqual({ kind: 'none', note: 'Epic Games doesn’t keep saves for this game.' });
  });

  it('a conflict whose cloud side has no size leaves it out', () => {
    const c = { state: 'conflict', synced_at: 0, conflict: { local_at: NOW - 60, local_bytes: 2 * 1024 ** 2, local_files: 3, cloud_at: NOW - 120, cloud_bytes: null, cloud_files: null } };
    const t = tvCloudTab(epicInstalled({ cloud: 'conflict' }), c as unknown as StoreCloudState, NOW, false);
    expect(t.kind).toBe('conflict');
    if (t.kind === 'conflict') {
      expect(t.cloud.sub).toMatch(/on another computer · replaces the save here$/);
      expect(t.local.sub).toMatch(/uploads it to Epic Games$/);
    }
  });

  it('Settings › Stores: the row, the tool row, the not-connected note and the Disconnect words', () => {
    expect(storeRowSub(epicStatus())).toBe('Signed in as SamPlays');
    expect(storeRowSub(epicStatus({ tool: { ...status().tool, state: 'not_installed' } }))).toBe('Signed in as SamPlays · legendary not installed');
    expect(toolRow(epicStatus({ tool: { ...status().tool, state: 'installed', version: '0.21.1' } }))).toEqual({ info: true, value: 'legendary 0.21.1 · installed' });
    expect(toolRow(epicStatus({ tool: { ...status().tool, state: 'failed', error: '' } }))).toMatchObject({ label: 'Couldn’t install legendary', value: 'Try again' });
    expect(notConnectedNote(epicStatus({ connection: 'not_connected' }))).toMatch(
      /^Sign in to Epic Games to see the games you own in the Library and install them\. GRID installs with legendary \(legendary, GPL-3\.0, .+\), which it downloads when you connect\.$/,
    );
    expect(disconnectWords(epicStatus({ installed_count: 2 }))).toEqual({
      title: 'Disconnect Epic Games?',
      body: 'GRID deletes your Epic Games login and legendary from this computer. The 2 installed games stay and still play; they can’t install, update or sync cloud saves until you connect again.',
      ok: 'Disconnect',
    });
  });

  it('Disconnect raises the prototype’s card', () => {
    expect(disconnectedCard('epic')).toEqual({ text: 'Epic Games disconnected', sub: 'Its login and legendary were deleted; installed games stay.' });
  });

  it('the sign-in shows Epic’s login address', () => {
    expect(loginHost('epic')).toBe('www.epicgames.com/id/login');
  });

  it('a Downloads row’s list line names Epic Games', () => {
    expect(tvStoreListSub(row({ source_id: 'epic:Glimmer', platform: 'Epic Games', kind: 'store_update' }))).toMatch(/^Epic Games/);
  });
});
