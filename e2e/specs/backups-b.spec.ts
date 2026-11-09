import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { driveRoot, mountOf, packDir, plugIn, readPack, unplug } from '../helpers/drives.js';
import {
  backupRows,
  driveStatus,
  openBackupDrives,
  recordedToasts,
  show,
  startRecording,
  waitForRecordedToast,
  type Row,
} from '../helpers/backups.js';
import { openPlatform } from '../helpers/server.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `backups/backups-b`: the drive is pulled out mid-backup, then
 * plugged back in. Runs on spec a's profile (the session restores from the
 * keyring) and on spec a's TESTDRIVE, which already holds two games, PCSX2
 * and the PS2 firmware, so this backup is four game rows and nothing else.
 *
 * The four games' files carry their own `e2e_throttle` (fixtures-backups),
 * and the first, "Big Arcade Game", is 2 MB: about ten seconds of download,
 * during which the drive goes away.
 */
describe('backups-b: the drive is removed mid-backup', () => {
  const DRIVE = 'TESTDRIVE';
  const GAMES = [903, 904, 905, 906];

  before(async () => {
    // The restored session opens on Server.
    await $(testId('platform-btn-1')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the restored session never rendered the Server view',
    });
    await openBackupDrives(DRIVE);
    await browser.waitUntil(async () => (await driveStatus(DRIVE)) === 'Ready for backups', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `${DRIVE} from spec a is not ready (it reads "${await driveStatus(DRIVE)}")`,
    });
  });

  it('queues four games to the drive', async () => {
    await show('server');
    await openPlatform(1);
    await $(testId('server-select')).click();
    for (const id of GAMES) await $(testId(`game-card-${id}`)).click();
    await expect($(testId('server-selection-count'))).toHaveText(expect.stringContaining('4 selected'));
    await $(testId('server-selection-backup')).click();
    await $(testId('backup-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const start = $(testId('backup-start'));
    await browser.waitUntil(async () => (await start.isEnabled()) && (await start.getText()) === 'Backup 4 games', {
      timeout: 15_000,
      timeoutMsg: 'the Backup dialog never offered "Backup 4 games"',
    });
    // Everything else is already on the drive.
    await expect($(testId('backup-emulators'))).toHaveText('Already on the drive');
    await startRecording();
    await start.click();
    await $(testId('backup-dialog')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
  });

  it('fails the row in progress and cancels the rest as one row when the drive is pulled', async () => {
    await show('downloads');
    let rows: Row[] = [];
    await browser
      .waitUntil(
        async () => {
          rows = await backupRows();
          const big = rows.find((r) => r.title === 'Big Arcade Game');
          return big !== undefined && big.detail.startsWith('Downloading from server');
        },
        { timeout: 20_000, interval: 100 },
      )
      .catch(() => {
        throw new Error(`"Big Arcade Game" never started downloading: ${JSON.stringify(rows)}`);
      });
    unplug(DRIVE);

    const removed = `${DRIVE} was removed`;
    await browser
      .waitUntil(
        async () => {
          rows = await backupRows();
          return rows.some((r) => r.title === 'Big Arcade Game' && r.detail === `Failed — ${removed}`) &&
            rows.some((r) => r.detail.startsWith('Cancelled'));
        },
        { timeout: 15_000, interval: 200 },
      )
      .catch(() => {
        throw new Error(`the removal never failed and cancelled the rows: ${JSON.stringify(rows)}`);
      });
    const failed = rows.filter((r) => r.detail.startsWith('Failed'));
    expect(failed.map((r) => r.title)).toEqual(['Big Arcade Game']);
    const cancelled = rows.filter((r) => r.detail.startsWith('Cancelled'));
    expect(cancelled).toHaveLength(1);
    expect(cancelled[0].title).toBe('3 more items');
    expect(cancelled[0].detail).toBe(`Cancelled — ${removed} · backup again to finish`);
    // No row of this backup is still waiting or running.
    expect(rows.filter((r) => /^(Waiting|Downloading|Writing)/.test(r.detail))).toEqual([]);

    const card = await waitForRecordedToast(new RegExp(`^${removed}\\.`));
    expect(card).toBe(`${removed}. 0 of 4 items were written to ${DRIVE}.`);
    await browser.pause(1500);
    const toasts = await recordedToasts();
    expect(toasts.filter((t) => t.includes(removed))).toHaveLength(1);
    expect(toasts.filter((t) => / backed up to /.test(t))).toEqual([]);
  });

  it('removes a leftover temporary file when the drive comes back, and the index stays valid', async () => {
    // The drive is away as `.TESTDRIVE`: give it a half-written file.
    const away = path.join(driveRoot(), `.${DRIVE}`, 'gridpack', 'games', 'x');
    mkdirSync(away, { recursive: true });
    writeFileSync(path.join(away, 'foo.zip.gridtmp'), 'half a file');
    plugIn(DRIVE);
    const leftover = path.join(packDir(DRIVE), 'games', 'x', 'foo.zip.gridtmp');
    expect(existsSync(leftover)).toBe(true);

    await browser.waitUntil(() => !existsSync(leftover), {
      timeout: 15_000,
      timeoutMsg: `the leftover ${leftover} was never removed`,
    });
    // Still a pack GRID can read, with only spec a's two games.
    const pack = readPack(DRIVE);
    expect(pack.games.map((g) => g.source.rom_id).sort()).toEqual([901, 902]);
    for (const game of pack.games) {
      for (const file of game.files) expect(existsSync(path.join(packDir(DRIVE), file.path))).toBe(true);
    }
    expect(readdirSync(mountOf(DRIVE))).toContain('gridpack');

    await openBackupDrives(DRIVE);
    await browser.waitUntil(async () => (await driveStatus(DRIVE)) === 'Ready for backups', {
      timeout: 10_000,
      timeoutMsg: `${DRIVE} did not come back as ready (it reads "${await driveStatus(DRIVE)}")`,
    });
  });
});
