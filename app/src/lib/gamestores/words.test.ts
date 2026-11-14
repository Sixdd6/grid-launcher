import { describe, expect, it } from 'vitest';
import { rommInstallerLine, storeBackupLine, storeEntryLine, storeKindLabel, storeStripLine } from './downloads';
import { installerFailedNotice, loginFailedNotice, noticeActionLabel, uninstalledNotice, wizardNotice } from './notices';
import { inspectionView, installerNeedBytes, installerSpaceNote, readingLine } from './inspection';
import { initialChoice, installDialogView, installRequest } from './install';
import {
  chosenExecutable,
  cloudPanel,
  conflictSides,
  executableChoices,
  storeButtons,
  storeDetailChips,
  storeFacts,
  storeLines,
  storeTabs,
  driveRestoreLine,
} from './details';
import { accountLine, disconnectConfirm, ownedLine, storePill, toolLine } from './settings';
import { storeBackupTitle, storeBackupView } from './backup';
import { FakeStores } from './fakeApi';
import { installedGame, row, status, storeGame } from './fixtures.test-util';
import type { InstallerInspection, StoreBackupPlan } from '../api';

const GB = 1024 ** 3;

describe('Downloads lines', () => {
  it('words every live store phase', () => {
    expect(storeEntryLine(row())).toBe('Downloading 45% · 41.2 MB/s');
    expect(storeEntryLine(row({ status: 'queued' }))).toBe('Waiting for a slot');
    expect(storeEntryLine(row({ status: 'installing', phase: 'installing', install_processed_bytes: 9, install_total_bytes: 10 }))).toBe('Installing 90%');
    expect(storeEntryLine(row({ kind: 'store_repair', phase: 'verifying' }))).toBe('Verifying files 45%');
    expect(storeEntryLine(row({ kind: 'store_tool', phase: 'checksum', downloaded_bytes: 40 }))).toBe('Downloading 40% · checking its SHA-256');
    expect(storeEntryLine(row({ kind: 'store_installer', phase: 'unpacking' }))).toBe('Unpacking the game files 45%');
    expect(storeEntryLine(row({ kind: 'store_installer', phase: 'installer_silent', downloaded_bytes: 30 }))).toBe('Running the installer silently · 30%');
    expect(storeEntryLine(row({ kind: 'store_installer', phase: 'installer_wizard' }))).toBe('Waiting for the installer’s setup window · finish it there');
    expect(storeEntryLine(row({ kind: 'store_restore', phase: 'copying_installer', drive_label: 'SAMSUNG T7', downloaded_bytes: 60 }))).toBe('Copying the installer from SAMSUNG T7 60%');
    expect(storeEntryLine(row({ kind: 'store_restore', phase: 'running_installer', downloaded_bytes: 40 }))).toBe('Running the installer · 40%');
    expect(storeEntryLine(row({ kind: 'store_restore', phase: 'copying_from_drive', drive_label: 'SAMSUNG T7' }))).toBe('Copying from SAMSUNG T7 45%');
  });
  it('words completed, failed and cancelled rows', () => {
    expect(storeEntryLine(row({ status: 'completed', install_total_bytes: 26 * GB }))).toBe('Installed · 26.0 GB');
    expect(storeEntryLine(row({ status: 'completed', kind: 'store_update' }), { version: '2.2' })).toBe('Updated to 2.2');
    expect(storeEntryLine(row({ status: 'completed', kind: 'store_repair', detail: '2 files repaired' }))).toBe('Verified · 2 files repaired');
    expect(storeEntryLine(row({ status: 'completed', kind: 'store_installer' }))).toBe('Installed from its installer');
    expect(storeEntryLine(row({ status: 'completed', kind: 'store_restore', phase: 'running_installer', drive_label: 'SAMSUNG T7' }))).toBe(
      'Installed from SAMSUNG T7 · ran its installer · saves restored',
    );
    expect(storeEntryLine(row({ status: 'completed', kind: 'store_restore', phase: 'copying_from_drive', drive_label: 'SAMSUNG T7' }))).toBe('Copied from SAMSUNG T7 · saves restored');
    expect(storeEntryLine(row({ status: 'cancelled' }))).toBe('Cancelled');
    expect(storeEntryLine(row({ status: 'failed', error: 'gogdl stopped: GOG’s download server didn’t answer' }))).toBe(
      'Failed · gogdl stopped: GOG’s download server didn’t answer',
    );
    // grid-core's own words for a failed silent install.
    expect(
      storeEntryLine(
        row({
          status: 'failed',
          kind: 'store_installer',
          error: "Couldn't run the installer silently: there isn't enough disk space. The installer was kept.",
          failure: { code: 'installer_silent_failed', base_title: '', base_gog_id: '', kept: true, reason: "there isn't enough disk space" },
        }),
      ),
    ).toBe("Failed · Couldn't run the installer silently: there isn't enough disk space. The installer was kept.");
  });
  it('names the kind badges', () => {
    expect(storeKindLabel('store_install')).toBe('');
    expect(storeKindLabel('store_dlc')).toBe('Content');
    expect(storeKindLabel('store_tool')).toBe('Store tool');
    expect(storeKindLabel('store_installer')).toBe('Installer');
    expect(storeKindLabel('backup_store_game')).toBe('Backup');
    expect(storeKindLabel('base')).toBeNull();
  });
  it('words a RomM install that runs its archive’s GOG installer', () => {
    const fake = new FakeStores();
    expect(rommInstallerLine(fake.rommInstallerRow('Grey Meridian', 'Windows', 'installer_silent', 0.4), { compat: 'GE-Proton10-17' })).toBe(
      'Windows · Running the GOG installer silently under GE-Proton10-17 · 40%',
    );
    expect(rommInstallerLine(fake.rommInstallerRow('Grey Meridian', 'Windows', 'installer_silent', 0.4), { compat: '' })).toBe(
      'Windows · Running the GOG installer silently · 40%',
    );
    expect(rommInstallerLine(fake.rommInstallerRow('Paper Orbit', 'Linux', 'unpacking', 0.4))).toBe('Linux · Unpacking the GOG installer 40%');
    expect(rommInstallerLine(fake.rommInstallerRow('Foxglove', 'Windows', 'installer_wizard', 0.7))).toBe(
      'Windows · Waiting for the installer’s setup window · finish it there',
    );
    expect(rommInstallerLine(fake.rommInstallerRow('X', 'Windows', 'downloading', 0.4))).toBeNull();
    const dlc = fake.failedInstallerRow('Low Tide Radio', 'Windows', { code: 'installer_dlc_base_missing', base_title: 'Pale Summit', base_gog_id: '9', kept: false, reason: '' }, '');
    expect(rommInstallerLine(dlc)).toBe(
      'Windows · Failed · Its archive holds a DLC installer for Pale Summit, which isn’t installed. Install Pale Summit first; the download was removed.',
    );
    const unk = fake.failedInstallerRow('Iron Lullaby', 'Windows', { code: 'installer_unrecognised', base_title: '', base_gog_id: '', kept: false, reason: '' },
      'Its archive holds an installer GRID doesn’t recognise (setup.exe has no GOG game info). The download was removed.');
    expect(rommInstallerLine(unk)).toBe(
      'Windows · Failed · Its archive holds an installer GRID doesn’t recognise (setup.exe has no GOG game info). The download was removed.',
    );
  });
  it('words GOG backup rows and the strip', () => {
    const b = row({ job: 'backup', kind: 'backup_store_game', detail: 'Installer files and saves', status: 'downloading', drive_label: 'SAMSUNG T7', drive_name: 'Couch kit' });
    expect(storeBackupLine(b)).toBe('GOG · Installer files and saves · Reading its GOG details');
    expect(storeBackupLine({ ...b, status: 'installing', install_processed_bytes: 45, install_total_bytes: 100 })).toBe(
      'GOG · Installer files and saves · Copying from this computer to SAMSUNG T7 45%',
    );
    expect(storeBackupLine({ ...b, status: 'completed' })).toBe('GOG · Installer files and saves · Copied from this computer · Written to Couch kit');
    expect(storeStripLine(row({ downloaded_bytes: 63 }))).toBe('Copper Saints · GOG · Downloading 63% · 41.2 MB/s');
  });
});

describe('notices', () => {
  it('labels every action', () => {
    expect(noticeActionLabel({ kind: 'play' })).toBe('Play');
    expect(noticeActionLabel({ kind: 'open_downloads' })).toBe('Open Downloads');
    expect(noticeActionLabel({ kind: 'try_again' })).toBe('Try again');
    expect(noticeActionLabel({ kind: 'sign_in' })).toBe('Sign in again');
    expect(noticeActionLabel({ kind: 'install_tool' }, 'gog')).toBe('Install gogdl');
    expect(noticeActionLabel({ kind: 'show_updates' })).toBe('Show');
    expect(noticeActionLabel({ kind: 'install_base', title: 'Pale Summit' })).toBe('Install Pale Summit');
  });
  it('words the frontend’s own cards', () => {
    expect(uninstalledNotice('gog', 'Ashen Tide', true).text).toBe('Ashen Tide uninstalled. Its cloud saves stay in your GOG account.');
    expect(loginFailedNotice('gog', 'Sign-in didn’t finish: GOG returned an error. Try again.')).toMatchObject({
      text: 'Couldn’t sign in to GOG: GOG returned an error. Try again.',
      error: true,
      action: { kind: 'try_again' },
    });
    expect(wizardNotice('Foxglove', false).text).toBe(
      'Foxglove’s installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.',
    );
    expect(wizardNotice('Quarry Nine', true).text).toMatch(/^The Quarry Nine installer couldn’t run silently/);
  });
  it('words RomM installer failures with Install <base>', () => {
    const fake = new FakeStores();
    const dlc = fake.failedInstallerRow('Low Tide Radio', 'Windows', { code: 'installer_dlc_base_missing', base_title: 'Pale Summit', base_gog_id: '', kept: false, reason: '' }, '');
    expect(installerFailedNotice(dlc)).toMatchObject({
      text: 'Couldn’t install Low Tide Radio: it’s DLC for Pale Summit, which isn’t installed',
      action: { kind: 'install_base', title: 'Pale Summit' },
    });
    const unk = fake.failedInstallerRow('Iron Lullaby', 'Windows', { code: 'installer_unrecognised', base_title: '', base_gog_id: '', kept: false, reason: '' }, '');
    expect(installerFailedNotice(unk)).toMatchObject({
      text: 'Couldn’t install Iron Lullaby: its archive holds an installer GRID doesn’t recognise',
      action: { kind: 'open_downloads' },
    });
  });
});

describe('offline installer card', () => {
  const fake = new FakeStores();
  const insp = (path: string): InstallerInspection => fake.handle('offline_inspect', { path }) as InstallerInspection;
  it('reads the file', () => {
    expect(readingLine('/home/p/Downloads/x.sh')).toBe('Looking for the game’s goggame info in x.sh');
  });
  it('words a Linux and a Windows installer', () => {
    const lin = inspectionView(insp('/home/player/Downloads/paper_lanterns_1_1_0_60321.sh'), { compat: 'GE-Proton10-17', gogConnected: true });
    expect(lin).toMatchObject({
      kind: 'found',
      heading: 'Install Paper Lanterns',
      what: 'Linux version · v1.1.0 · 1.1 GB',
      from: 'From a GOG offline installer · paper_lanterns_1_1_0_60321.sh',
      how: 'GRID unpacks the game files from it. The installer script isn’t run.',
      showFolder: true,
      showCompat: false,
      button: 'Install',
    });
    const win = inspectionView(insp('/home/player/Downloads/setup_quarry_nine_1.4_(64bit)_(69001).exe'), { compat: 'GE-Proton10-17', gogConnected: false });
    expect(win.kind === 'found' && win.how).toMatch(/^GRID runs the installer silently under GE-Proton10-17\./);
    expect(win.kind === 'found' && win.joins).toBe('It joins GOG › Installed. Sign in to GOG later for its updates and cloud saves.');
  });
  it('words an update and a DLC', () => {
    const up = inspectionView(insp('/home/player/Downloads/ashen_tide_1_0_5_71240.sh'), { compat: '', gogConnected: true });
    expect(up).toMatchObject({
      heading: 'Update Ashen Tide to 1.0.5',
      already: 'Ashen Tide is installed already (v1.0.4). Installing this replaces its files with v1.0.5. Saves and settings stay.',
      button: 'Update',
    });
    const dlc = inspectionView(insp('/home/player/Downloads/ashen_tide_digital_artbook_71299.sh'), { compat: '', gogConnected: true });
    expect(dlc).toMatchObject({ heading: 'Install DLC for Ashen Tide', what: 'Digital Artbook · DLC for Ashen Tide · 118 MB', button: 'Install DLC' });
  });
  it('says what it needs and what is free, as the prototype’s footer', () => {
    const win = insp('/home/player/Downloads/setup_quarry_nine_1.4_(64bit)_(69001).exe');
    expect(installerSpaceNote(win, win.free_bytes)).toBe('Needs 13.2 GB · 412 GB free');
    expect(installerSpaceNote(win, null)).toBe('Needs 13.2 GB');
    expect(installerNeedBytes(100)).toBe(140);
  });
  it('passes refusals through', () => {
    const r = inspectionView(insp('/home/player/Downloads/ashen_tide_1.0.4_71234.pkg'), { compat: '', gogConnected: true });
    expect(r).toMatchObject({ kind: 'refused', heading: 'Can’t install this file', title: 'This is a macOS installer' });
  });
});

describe('install dialog', () => {
  const fake = new FakeStores();
  const mk = fake.games.find((g) => g.store_id === '1006')!;
  const opts = fake.installOptions(mk);
  it('defaults to the Linux build, the default language and every DLC', () => {
    const c = initialChoice(opts);
    expect(c.platform).toBe('linux');
    expect(c.language).toBe('English');
    expect(c.dlcIds).toEqual(['2003', '2004']);
  });
  it('words the dialog and turns Install into Not enough space', () => {
    const c = initialChoice(opts);
    const v = installDialogView(mk, 'GOG', opts, c);
    expect(v.title).toBe('Install Marrowind Keep');
    expect(v.sub).toBe('GOG · 22.0 GB download');
    expect(v.versions.map((r) => r.note)).toEqual(['Runs natively on this computer', 'Runs through GE-Proton10-17']);
    expect(v.showLanguage).toBe(true);
    expect(v.showCompat).toBe(false);
    expect(v.footNote).toBe('24.7 GB to download');
    expect(v.spaceLine).toMatch(/ on disk · 412 GB free on \/home$/);
    const small = installDialogView(mk, 'GOG', { ...opts, free_bytes: 18.4 * GB }, c);
    expect(small).toMatchObject({ tooBig: true, button: 'Not enough space' });
    expect(small.spaceLine).toMatch(/^Needs .* · 18.4 GB free on \/home$/);
    expect(installRequest(mk, { ...c, platform: 'windows' }).compat_tool).toBe('GE-Proton10-17');
    expect(installRequest(mk, c).compat_tool).toBe('');
  });
});

describe('Details', () => {
  const now = 100_000;
  it('lists the buttons for an installed game with an update', () => {
    const g = installedGame({ update_version: '2.2' });
    expect(storeButtons({ game: g, status: status(), job: null, playing: false, confirm: null }).map((b) => b.label)).toEqual([
      'Play',
      'Update to 2.2',
      'Game Settings',
      'Backup to drive…',
      'Verify and repair',
      'Uninstall',
    ]);
    expect(storeButtons({ game: g, status: status(), job: null, playing: true, confirm: 'uninstall' }).map((b) => b.label)).toContain('Confirm uninstall');
    expect(storeButtons({ game: g, status: status(), job: null, playing: true, confirm: null })[0].label).toBe('Stop');
  });
  it('lists Cancel while a job runs, Install… disabled while signed out', () => {
    expect(storeButtons({ game: storeGame(), status: status(), job: row(), playing: false, confirm: null })[0].label).toBe('Cancel install · 45%');
    expect(storeButtons({ game: storeGame(), status: status(), job: row({ status: 'queued' }), playing: false, confirm: null })[0].label).toBe('Cancel install · queued');
    expect(storeButtons({ game: storeGame(), status: status({ connection: 'expired' }), job: null, playing: false, confirm: null })[0]).toMatchObject({
      label: 'Install…',
      disabled: true,
    });
  });
  it('offers Install from drive and Remove from drive… from a drive', () => {
    const dg = { key: 'gog:1006', name: 'Marrowind Keep', platform: 'linux' as const, layout: 'installer' as const, version: '1.2', size: 1, cover: '', installed: false, added: '', summary: '' };
    const b = storeButtons({ game: storeGame({ key: 'gog:1006' }), status: status(), job: null, playing: false, confirm: null, drive: { name: 'Couch kit', game: dg } });
    expect(b.map((x) => x.label)).toEqual(['Install from drive', 'Remove from drive…']);
    expect(driveRestoreLine(dg)).toBe('On the drive as its installer files · v1.2 and saves. Installing runs the installer again.');
    expect(driveRestoreLine({ ...dg, layout: 'installed', version: '1.1.0' })).toBe(
      'On the drive as a copy of an installed game · v1.1.0, with its saves. Installing copies it back.',
    );
  });
  it('writes the lines under the buttons', () => {
    const conflict = installedGame({ cloud: 'conflict', installed_platform: 'windows', compat_tool: 'GE-Proton10-17', origin: 'offline' });
    const lines = storeLines(
      { game: conflict, status: status(), job: null, playing: false, confirm: null },
      { nowSeconds: now, lastPlayedAt: now - 86400, compat: 'GE-Proton10-17', onDrives: ['Couch kit'], syncing: false },
    );
    expect(lines.map((l) => l.text)).toEqual([
      'Last played yesterday',
      'Runs through GE-Proton10-17 · Windows version',
      'Cloud save conflict · ',
      'Installed from an offline installer',
      'On Couch kit',
    ]);
    const expired = storeLines(
      { game: storeGame(), status: status({ connection: 'expired' }), job: null, playing: false, confirm: null },
      { nowSeconds: now, lastPlayedAt: 0, compat: 'GE-Proton10-17', onDrives: [], syncing: false },
    );
    expect(expired[0]).toMatchObject({ text: 'Your GOG login expired.', link: { label: 'Sign in again' } });
  });
  it('shows the chips, tabs and facts', () => {
    const g = installedGame({ update_version: '1.1', origin: 'offline' });
    expect(storeDetailChips(g, true).map((c) => c.text)).toEqual(['GOG', 'Playing', 'v1.0.4', 'Linux version', 'Offline installer']);
    expect(storeTabs(g)).toEqual(['overview', 'media', 'cloud']);
    expect(storeTabs(storeGame())).toEqual(['overview', 'media']);
    expect(storeFacts(g).find((f) => f.term === 'Version')?.value).toBe('v1.0.4 · GOG v1.1');
  });
  it('words the cloud tab and the conflict', () => {
    const g = installedGame();
    expect(cloudPanel(g, { state: 'synced', synced_at: now - 7200, locations: [], conflict: null }, now)).toMatchObject({
      kind: 'synced',
      title: 'Synced 2 hours ago',
      button: 'Sync now',
    });
    expect(cloudPanel(g, { state: 'syncing', synced_at: 0, locations: [], conflict: null }, now)).toMatchObject({ title: 'Syncing…', busy: true });
    expect(cloudPanel(g, { state: 'off', synced_at: 0, locations: [], conflict: null }, now).title).toBe('Cloud saves are off for this game');
    expect(cloudPanel(g, { state: 'none', synced_at: 0, locations: [], conflict: null }, now).title).toBe('This game has no cloud saves');
    const c = { local_at: now - 60, local_files: 3, local_bytes: 2.1 * 1024 * 1024, cloud_at: now - 7200, cloud_files: 3, cloud_bytes: 2.0 * 1024 * 1024 };
    const sides = conflictSides('gog', c, now);
    expect(sides[0]).toMatchObject({ label: 'This computer', button: 'Keep this computer’s save', after: 'Uploads it to GOG.', newer: true });
    expect(sides[0].note).toMatch(/· 3 files · 2.1 MB$/);
    expect(sides[1]).toMatchObject({ label: 'GOG cloud', button: 'Use the cloud save', newer: false });
    expect(sides[1].note).toMatch(/ on another computer · 3 files · 2.0 MB$/);
    // The prompt shows the size only, as the prototype's conflict prompt does.
    expect(conflictSides('gog', c, now, true)[1].note).toMatch(/ on another computer · 2.0 MB$/);
    // A size GOG's listing didn't give is left out.
    const unsized = { ...c, cloud_files: 1, cloud_bytes: null };
    expect(conflictSides('gog', unsized, now)[1].note).toMatch(/ on another computer · 1 file$/);
    expect(conflictSides('gog', unsized, now, true)[1].note).toMatch(/ on another computer$/);
  });
});

describe('Settings › Stores', () => {
  const now = 10_000;
  it('words the pill, account and owned lines', () => {
    expect(storePill(status())).toEqual({ text: 'Connected', tone: 'ok' });
    expect(storePill(status({ connection: 'expired' })).text).toBe('Login expired');
    expect(accountLine(status())).toBe('Signed in as sam_plays');
    expect(accountLine(status({ connection: 'not_connected', installed_count: 3 }))).toBe('Not signed in · 3 games installed here');
    expect(ownedLine(status({ owned_count: 8, installed_count: 3, refreshed_at: now - 300 }), now)).toBe('8 · 3 installed · updated 5 min ago');
    expect(ownedLine(status({ owned: 'loading' }), now)).toBeNull();
  });
  it('words the tool row in each state', () => {
    expect(toolLine(status())).toEqual({ kind: 'installed', text: 'gogdl 1.4.0 · installed' });
    expect(toolLine(status({ tool: { state: 'not_installed' } as never })).kind).toBe('missing');
    expect(toolLine(status({ tool: { state: 'installing', done: 1, total: 2 } as never }))).toMatchObject({ kind: 'installing', text: 'Installing gogdl…', pct: 50 });
    expect(toolLine(status({ tool: { state: 'failed', error: 'checksum' } as never })).text).toBe(
      'Couldn’t install gogdl: the download didn’t match its checksum, so GRID deleted it.',
    );
    expect(toolLine(status({ tool: { state: 'failed', error: 'needs Python 3.10' } as never })).kind).toBe('python');
    expect(toolLine(status({ connection: 'not_connected', tool: { state: 'not_installed' } as never })).text).toMatch(/^Installs when you connect · /);
  });
  it('words the Disconnect confirm', () => {
    expect(disconnectConfirm(status({ installed_count: 3 }))).toEqual({
      bold: 'Disconnect GOG?',
      rest: 'GRID deletes your GOG login and gogdl from this computer. The 3 installed games stay and still play; they can’t install, update or sync cloud saves until you connect again.',
    });
  });
});

describe('GOG backups', () => {
  const plan: StoreBackupPlan = {
    drive: '/media/t7',
    rows: [
      { key: 'gog:1003', name: 'Brightwater', layout: 'installer', detail: 'brightwater_1_3_58810.sh · 2.9 GB, and its saves', bytes: 2.9 * GB, windows_prefix_note: false, already: false, refused: '' },
      { key: 'gog:1001', name: 'Ashen Tide', layout: 'installed', detail: 'Installed with gogdl…', bytes: 21.6 * GB, windows_prefix_note: true, already: false, refused: '' },
      { key: 'gog:1006', name: 'Marrowind Keep', layout: 'installer', detail: '', bytes: 0, windows_prefix_note: false, already: true, refused: '' },
      { key: 'gog:1004', name: 'Copper Saints', layout: 'installed', detail: '', bytes: 0, windows_prefix_note: false, already: false, refused: 'Copper Saints is being installed or updated. Wait for it to finish.' },
    ],
    bytes_to_write: 24.5 * GB,
    free_bytes: 230 * GB,
    fits: true,
  };
  it('lists rows, skipped games, refusals and the button', () => {
    const v = storeBackupView(plan, 'Couch kit');
    expect(v.rows.map((r) => [r.name, r.tag])).toEqual([
      ['Brightwater', 'Installer files'],
      ['Ashen Tide', 'Installed copy'],
    ]);
    expect(v.rows[0].note).toBe('GOG · brightwater_1_3_58810.sh · 2.9 GB, and its saves');
    expect(v.windowsNote).toBe(true);
    expect(v.skipped).toBe('1 game is already on Couch kit and will be skipped: Marrowind Keep.');
    expect(v.refusals).toEqual(['Copper Saints is being installed or updated. Wait for it to finish.']);
    expect(v.footNote).toBe('24.5 GB to write · nothing to download');
    expect(v.button).toBe('Backup 2 games');
    expect(storeBackupView({ ...plan, fits: false }, 'Couch kit').button).toBe('Not enough space');
    expect(storeBackupView({ ...plan, rows: [] }, 'Couch kit')).toMatchObject({ button: 'Nothing to backup', canStart: false });
  });
  it('titles the dialog', () => {
    expect(storeBackupTitle({ kind: 'all', count: 3 })).toBe('Backup GOG · 3 games');
    expect(storeBackupTitle({ kind: 'game', name: 'Ashen Tide' })).toBe('Backup Ashen Tide');
  });
});

describe('Game Settings › Executable', () => {
  const dir = '/games/GOG/Lanternfall';
  const found = [`${dir}/Lanternfall.exe`, `${dir}/bin/Launcher.exe`];
  it('lists the folder’s executables by their path inside it', () => {
    expect(executableChoices(`${dir}/Lanternfall.exe`, found, dir)).toEqual([
      { value: `${dir}/Lanternfall.exe`, label: 'Lanternfall.exe' },
      { value: `${dir}/bin/Launcher.exe`, label: 'bin/Launcher.exe' },
    ]);
  });
  it('keeps a pinned file the list doesn’t hold, first', () => {
    const got = executableChoices('/elsewhere/Mod.exe', found, dir);
    expect(got[0]).toEqual({ value: '/elsewhere/Mod.exe', label: '/elsewhere/Mod.exe' });
    expect(got).toHaveLength(3);
    expect(executableChoices('', [], dir)).toEqual([]);
  });
  it('matches the current file across separators', () => {
    const win = ['C:\\Games\\Lanternfall\\Lanternfall.exe'];
    expect(executableChoices('C:/Games/Lanternfall/Lanternfall.exe', win, 'C:\\Games\\Lanternfall')).toEqual([
      { value: win[0], label: 'Lanternfall.exe' },
    ]);
    expect(chosenExecutable('C:/Games/Lanternfall/Lanternfall.exe', executableChoices('', win, ''))).toBe(win[0]);
  });
});
