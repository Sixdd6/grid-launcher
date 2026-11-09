import { existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { discard, FAKE_DRIVE_FILE, mountOf, packDir, plugIn, readPack, tryReadPack } from '../helpers/drives.js';
import {
  backupRows,
  driveCard,
  driveStatus,
  openBackupDrives,
  recordedToasts,
  show,
  startRecording,
  waitForRecordedToast,
} from '../helpers/backups.js';
import { openPlatform } from '../helpers/server.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Every file and folder under `dir`, relative to it, `/`-separated. */
function walk(dir: string, prefix = ''): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const rel = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
    return entry.isDirectory() ? [rel, ...walk(path.join(dir, entry.name), rel)] : [rel];
  });
}

/**
 * Stage `backups/backups-a`: set up a fake drive, back up two games from
 * Server select mode, and check what landed on the drive from Node.
 *
 * The seed makes PCSX2 (installed from the server's Emulators platform)
 * PlayStation 2's emulator, so the backup carries the server's PCSX2 builds
 * for both computers the pack is for, and the PS2 firmware.
 */
describe('backups-a: set up a drive and back up two games', () => {
  const DRIVE = 'TESTDRIVE';

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never rendered a platform button after connecting',
    });
  });

  it('lists a newly plugged-in drive as Not set up', async () => {
    plugIn(DRIVE, { fs: 'exfat' });
    await openBackupDrives(DRIVE);
    await browser.waitUntil(async () => (await driveStatus(DRIVE)) === 'Not set up', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `${DRIVE} never read "Not set up" (it reads "${await driveStatus(DRIVE)}")`,
    });
  });

  it('sets the drive up for Linux and Windows', async () => {
    await $(`${driveCard(DRIVE)} ${testId('backup-drive-setup')}`).click();
    const name = $(`${driveCard(DRIVE)} ${testId('backup-drive-setup-name')}`);
    await name.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await name.getValue()).toBe(DRIVE);
    // This computer's OS starts ticked; tick the other one.
    for (const os of ['linux', 'windows']) {
      const box = $(`${driveCard(DRIVE)} ${testId(`backup-drive-setup-os-${os}`)}`);
      if (!(await box.isSelected())) await box.click();
      expect(await box.isSelected()).toBe(true);
    }
    // Setup on a non-FAT drive has no 4 GB warning.
    expect(await $(`${driveCard(DRIVE)} ${testId('backup-drive-fat32')}`).isExisting()).toBe(false);
    await $(`${driveCard(DRIVE)} ${testId('backup-drive-setup-go')}`).click();

    await browser.waitUntil(async () => (await driveStatus(DRIVE)) === 'Ready for backups', {
      timeout: 10_000,
      timeoutMsg: `${DRIVE} never read "Ready for backups" (it reads "${await driveStatus(DRIVE)}")`,
    });
    await expect($(`${driveCard(DRIVE)} ${testId('backup-drive-for')}`)).toHaveText('Linux and Windows');

    const pack = readPack(DRIVE);
    expect(pack.format).toBe(1);
    expect(pack.name).toBe(DRIVE);
    expect([...pack.os].sort()).toEqual(['linux', 'windows']);
    expect(pack.games).toEqual([]);
  });

  it('backs up two games picked in Server select mode', async () => {
    await show('server');
    await openPlatform(1);
    await $(testId('server-select')).click();
    await expect($(testId('server-select'))).toHaveText('Done');
    await $(testId('game-card-901')).click();
    await $(testId('game-card-902')).click();
    await expect($(testId('server-selection-count'))).toHaveText(expect.stringContaining('2 selected'));
    // A pick never opens Details.
    expect(await $(testId('details-panel')).isExisting()).toBe(false);
    await $(testId('server-selection-backup')).click();

    const dialog = $(testId('backup-dialog'));
    await dialog.waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the Backup dialog never opened' });
    await expect($(testId('backup-dialog-title'))).toHaveText('Back up 2 games');
    // The plan: a size to write, the emulator for both computers, and the
    // firmware ticked by default.
    await browser.waitUntil(async () => /to write$/.test((await $(testId('backup-footer-note')).getText()).trim()), {
      timeout: 15_000,
      timeoutMsg: 'the Backup dialog never showed the size to write',
    });
    await expect($(testId('backup-emulators'))).toHaveText(expect.stringContaining('PCSX2'));
    await expect($(testId('backup-emulators'))).toHaveText(expect.stringContaining('Linux and Windows'));
    const firmware = $(testId('backup-firmware'));
    expect(await firmware.isSelected()).toBe(true);
    expect(await firmware.isEnabled()).toBe(true);
    await expect($(testId('backup-firmware-text'))).toHaveText(expect.stringContaining('PlayStation 2'));

    const start = $(testId('backup-start'));
    await start.waitForEnabled({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Back up never became enabled' });
    await expect(start).toHaveText('Back up 2 games');
    await startRecording();
    await start.click();
    await dialog.waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    // Queuing leaves select mode.
    await expect($(testId('server-select'))).toHaveText('Select');
  });

  it('writes every Backup row to the drive, with one queued and one done card', async () => {
    await show('downloads');
    let rows = await backupRows();
    await browser
      .waitUntil(
        async () => {
          rows = await backupRows();
          return rows.length >= 4 && rows.every((r) => r.detail === `Written to ${DRIVE}`);
        },
        { timeout: 60_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the Backup rows never all reached "Written to ${DRIVE}": ${JSON.stringify(rows)}`);
      });
    const titles = rows.map((r) => r.title);
    expect(titles).toEqual(expect.arrayContaining(['Cinder Peaks', 'Moss and Marrow']));

    await waitForRecordedToast(/backed up to TESTDRIVE$/);
    // Let any late duplicate surface before counting.
    await browser.pause(1500);
    const toasts = await recordedToasts();
    expect(toasts.filter((t) => t === `2 games added to the backup queue for ${DRIVE}`)).toHaveLength(1);
    expect(toasts.filter((t) => / added to the backup queue for /.test(t))).toHaveLength(1);
    expect(toasts.filter((t) => / backed up to /.test(t))).toHaveLength(1);
    expect(toasts.filter((t) => /was removed|couldn't be backed up|Couldn't back up/.test(t))).toEqual([]);
  });

  it('leaves a valid gridpack with both games, their files, the emulators and the firmware', async () => {
    const pack = tryReadPack(DRIVE);
    expect(pack).not.toBeNull();
    const games = pack!.games;
    expect(games.map((g) => g.source.rom_id).sort()).toEqual([901, 902]);
    for (const game of games) {
      expect(game.platform).toBe('ps2');
      expect(game.source.server.replace(/\/$/, '')).toBe(mockUrl().replace(/\/$/, ''));
      expect(game.files.length).toBeGreaterThan(0);
      for (const file of game.files) {
        // Relative to gridpack/, so the drive works under any mount point.
        expect(path.isAbsolute(file.path)).toBe(false);
        expect(file.path.split('/')).not.toContain('..');
        expect(file.path.startsWith('games/ps2/')).toBe(true);
        const onDisk = path.join(packDir(DRIVE), file.path);
        expect(existsSync(onDisk)).toBe(true);
        expect(statSync(onDisk).size).toBe(file.size);
        expect(file.server_file_id).toBeGreaterThan(0);
      }
    }
    const moss = games.find((g) => g.source.rom_id === 902)!;
    expect(moss.files.map((f) => f.name).sort()).toEqual(['disc1.bin', 'disc2.bin', 'game.m3u']);

    // PCSX2 for each computer the pack is for.
    const emulators = pack!.emulators;
    expect(emulators.map((e) => e.os).sort()).toEqual(['linux-x86_64', 'windows-x86_64']);
    for (const emulator of emulators) {
      expect(emulator.source_id).toBe('romm:ps2/pcsx2 (playstation 2)');
      expect(path.isAbsolute(emulator.path)).toBe(false);
      expect(existsSync(path.join(packDir(DRIVE), emulator.path))).toBe(true);
    }
    // The server's PS2 firmware.
    const firmware = pack!.firmware.find((f) => f.platform === 'ps2');
    expect(firmware?.files.map((f) => f.name)).toEqual(['ps2-bios.bin']);
    for (const file of firmware!.files) {
      expect(statSync(path.join(packDir(DRIVE), file.path)).size).toBe(file.size);
    }

    // GRID wrote nothing outside gridpack/ (the fake drive's own probe file
    // is the test's), and no temporary file is left.
    expect(readdirSync(mountOf(DRIVE)).sort()).toEqual([FAKE_DRIVE_FILE, 'gridpack'].sort());
    expect(walk(packDir(DRIVE)).filter((rel) => rel.endsWith('.gridtmp'))).toEqual([]);
  });

  it('shows "On TESTDRIVE" in Details for a backed-up game', async () => {
    await show('server');
    await $(testId('game-card-901')).click();
    await $(testId('details-panel')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Details never opened for rom 901',
    });
    await browser.waitUntil(async () => (await $(testId('details-on-drives')).isExisting()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Details never said the game is on a drive',
    });
    await expect($(testId('details-on-drives'))).toHaveText(`On ${DRIVE}`);
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
  });

  it('warns about the 4 GB limit when setting up a FAT32 drive', async () => {
    const FAT = 'FATDRIVE';
    plugIn(FAT, { fs: 'vfat' });
    try {
      await openBackupDrives(FAT);
      await expect($(`${driveCard(FAT)} ${testId('backup-drive-status')}`)).toHaveText('Not set up');
      await $(`${driveCard(FAT)} ${testId('backup-drive-setup')}`).click();
      const warning = $(`${driveCard(FAT)} ${testId('backup-drive-fat32')}`);
      await warning.waitForDisplayed({
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the FAT32 setup form never warned about 4 GB files',
      });
      await expect(warning).toHaveText(expect.stringContaining('4 GB'));
      await expect(warning).toHaveText(expect.stringContaining('exFAT'));
      // Never set up: GRID made nothing on it.
      expect(existsSync(path.join(mountOf(FAT), 'gridpack'))).toBe(false);
    } finally {
      discard(FAT);
    }
  });
});
