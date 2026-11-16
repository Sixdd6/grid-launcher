import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { packDir, readPack } from '../helpers/drives.js';
import { backupRows, show, startRecording } from '../helpers/backups.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** The games backups-seed.mjs `seedE` installs (kept in step with it). */
const ZIP = { romId: 901, title: 'Cinder Peaks' };
const NATIVE = { romId: 961, title: 'Tux Trail' };
const MULTI = { romId: 902, title: 'Moss and Marrow' };
const GONE = { romId: 903, title: 'Far Shore' };
const LOCAL_IDS = [ZIP.romId, NATIVE.romId, MULTI.romId];
/** The server the seeded config remembers (RomM is not connected). */
const SERVER = 'http://romm.e2e.invalid';
const DRIVE = 'TESTDRIVE';

/** One column of the registry row for `romId`, read with the sqlite3 CLI as other specs do. */
function registry(column: string, romId: number): string {
  return execFileSync(
    'sqlite3',
    [path.join(dataDir(), 'grid-launcher.db'), `SELECT ${column} FROM installed_games WHERE rom_id = ${romId};`],
    { encoding: 'utf-8' },
  ).trim();
}

/** The mock's live request log. */
async function requests(): Promise<{ method: string; path: string }[]> {
  return (await (await fetch(`${mockUrl()}/__e2e__/requests`)).json()) as { method: string; path: string }[];
}

/** The visible text of every element matching `selector`, whitespace folded. */
async function texts(selector: string): Promise<string[]> {
  return browser.execute(
    (sel) => Array.from(document.querySelectorAll(sel)).map((el) => (el.textContent ?? '').replace(/\s+/g, ' ').trim()),
    selector,
  );
}

/**
 * Stage `backups/backups-e` (docs/romm-storefront-plan.md Step 2, user
 * decision 8): Game backups with RomM not connected. The seed gives
 * backups-d's library and TESTDRIVE, a config that remembers a server but
 * is not connected (nothing is probed), and a fourth game whose files are
 * gone. The backup copies the three installs from this computer, lists the
 * fourth as one it can't backup, skips the server-hosted emulator and the
 * firmware, and one game is restored from the drive.
 */
describe('backups-e: backup installed games with RomM not connected and restore one', () => {
  let requestsBefore = 0;

  before(async () => {
    await $(testId('library-view')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'GRID never opened its Library with RomM not connected',
    });
    requestsBefore = (await requests()).length;
  });

  it('opens the Backup dialog with RomM off: installed copies only, and why one game cannot go', async () => {
    await show('library');
    await $(testId('library-rail-all')).click();
    for (const id of [...LOCAL_IDS, GONE.romId]) {
      await $(testId(`library-card-${id}`)).waitForDisplayed({
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `Library never showed the installed rom ${id}`,
      });
    }
    await $(testId('library-select')).click();
    for (const id of [...LOCAL_IDS, GONE.romId]) await $(testId(`library-card-${id}`)).click();
    await expect($(testId('library-selection-count'))).toHaveText(expect.stringContaining('4 selected'));
    const backup = $(testId('library-selection-backup'));
    expect(await backup.isEnabled()).toBe(true);
    await backup.click();

    await $(testId('backup-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the Backup dialog never opened' });
    await expect($(testId('backup-dialog-title'))).toHaveText('Backup 4 games');

    const start = $(testId('backup-start'));
    await browser.waitUntil(async () => (await start.getText()).trim() === 'Backup 3 games', {
      timeout: 20_000,
      timeoutMsg: `the Backup button never read "Backup 3 games" (it reads "${await start.getText()}")`,
    });
    await expect($(testId('backup-lede'))).toHaveText(
      'RomM isn’t connected, so GRID copies the games installed on this computer, with the art and details it has, to the drive’s gridpack folder. Nothing downloads.',
    );

    // Source: ticked and locked.
    const useLocal = $(testId('backup-use-local'));
    expect(await useLocal.isSelected()).toBe(true);
    expect(await useLocal.isEnabled()).toBe(false);
    await expect($(testId('backup-local-line'))).toHaveText(
      expect.stringMatching(/^3 games from this computer \(.+\) · without RomM, only installed copies$/),
    );
    const sources = await texts(testId('backup-source-row'));
    expect(sources).toHaveLength(3);
    expect(sources.every((r) => r.endsWith('From this computer'))).toBe(true);
    // No out-of-date choice: there is no server to compare with.
    expect(await $(testId('backup-outdated')).isExisting()).toBe(false);

    // Can't backup without RomM.
    await expect($(`${testId('backup-unavailable')} h4`)).toHaveText('Can’t backup without RomM · 1');
    const unavailable = await texts(testId('backup-unavailable-row'));
    expect(unavailable).toEqual([
      `${GONE.title} PlayStation 2 · Its files weren't found on this computer. Connect RomM to back it up from the server.`,
    ]);

    // Also copy: emulators from GRID's catalog only, firmware skipped.
    await expect($(testId('backup-emulators'))).toHaveText(
      expect.stringContaining('from GRID’s catalog; builds hosted on your RomM server are skipped'),
    );
    expect(await texts('[data-testid="backup-emulators"] ~ .note')).toContain('PCSX2 (Playstation 2) needs RomM');
    const firmware = $(testId('backup-firmware'));
    expect(await firmware.isSelected()).toBe(false);
    expect(await firmware.isEnabled()).toBe(false);
    // The PlayStation 2 BIOS GRID's catalog lists is skipped, by system name.
    await expect($(testId('backup-firmware-text'))).toHaveText('Needs RomM · skipped: PlayStation 2');
  });

  it('copies the three installs to the drive under the remembered server', async () => {
    await startRecording();
    await $(testId('backup-start')).click();
    await $(testId('backup-dialog')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });

    await show('downloads');
    let rows = await backupRows();
    await browser
      .waitUntil(
        async () => {
          rows = await backupRows();
          const games = rows.filter((r) => [ZIP.title, NATIVE.title, MULTI.title].includes(r.title));
          return games.length === 3 && games.every((r) => r.detail.endsWith(`Written to ${DRIVE}`));
        },
        { timeout: 60_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the Backup rows never all reached "Written to ${DRIVE}": ${JSON.stringify(rows)}`);
      });
    for (const title of [ZIP.title, NATIVE.title, MULTI.title]) {
      expect(rows.find((r) => r.title === title)?.detail).toContain('Copied from this computer');
    }
    // Nothing for the server-hosted emulator, the firmware or Far Shore.
    expect(rows.map((r) => r.title)).not.toContain(GONE.title);
    expect(rows.some((r) => r.title.startsWith('PCSX2'))).toBe(false);
    expect(rows.some((r) => r.title.endsWith('firmware'))).toBe(false);

    const pack = readPack(DRIVE);
    expect(pack.games.map((g) => g.source.rom_id).sort()).toEqual([...LOCAL_IDS].sort());
    for (const game of pack.games) {
      // The same identity a backup with RomM writes: the server and the rom id.
      expect(game.source.server).toBe(SERVER);
      const installed = game.installed!;
      expect(installed).toBeDefined();
      expect(installed.outdated).toBe(false);
      for (const part of installed.parts) {
        const onDisk = path.join(packDir(DRIVE), part.path);
        expect(existsSync(onDisk)).toBe(true);
        expect(statSync(onDisk).size).toBe(part.size);
      }
    }
    expect(pack.games.find((g) => g.source.rom_id === MULTI.romId)?.name).toBe(MULTI.title);
    expect(pack.emulators).toEqual([]);
    expect(pack.firmware).toEqual([]);

    // Nothing reached a RomM server.
    const since = (await requests()).slice(requestsBefore);
    expect(since.filter((r) => r.path.startsWith('/api/'))).toEqual([]);
  });

  it('restores an uninstalled game from that drive, still without RomM', async () => {
    const library = path.join(dataDir(), 'library');
    const oldDir = registry('multi_file_game_dir', MULTI.romId);
    expect(oldDir).not.toBe('');

    await show('library');
    await $(testId('library-rail-all')).click();
    if ((await $(testId('library-select')).getText()).trim() === 'Done') await $(testId('library-select')).click();
    const card = $(testId(`library-card-${MULTI.romId}`));
    await card.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await card.click();
    const panel = () => $(testId('details-panel'));
    await panel().waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Details never opened' });
    const uninstall = $(testId('details-uninstall'));
    await uninstall.waitForEnabled({ timeout: TRANSITION_TIMEOUT });
    await uninstall.click();
    await expect(uninstall).toHaveText('Confirm uninstall');
    await uninstall.click();
    await panel().waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Details never closed after uninstalling' });
    await browser.waitUntil(() => registry('rom_id', MULTI.romId) === '', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the uninstalled game stayed in the registry',
    });
    expect(existsSync(oldDir)).toBe(false);

    const driveRow = $('[data-testid^="library-rail-drive-"]');
    await driveRow.waitForExist({ timeout: 10_000, timeoutMsg: 'Library never listed the backup drive' });
    await driveRow.click();
    const driveCard = $(testId(`library-drive-card-${MULTI.romId}`));
    await driveCard.waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: "the drive's copy never showed in Library" });
    await driveCard.click();
    await panel().waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const install = $(testId('details-install'));
    await expect(install).toHaveText('Install from drive');
    await install.click();
    await browser.waitUntil(() => registry('rom_id', MULTI.romId) !== '', {
      timeout: 30_000,
      timeoutMsg: 'the restored game never reached the registry',
    });
    if (await panel().isExisting()) {
      await $(testId('details-close')).click();
      await panel().waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    }

    const dir = registry('multi_file_game_dir', MULTI.romId);
    expect(dir.startsWith(library + path.sep)).toBe(true);
    expect(readdirSync(dir).sort()).toEqual(['disc1.bin', 'disc2.bin', 'game.m3u']);
    // The restored row keeps the server it came from, so backing it up
    // again writes the same identity.
    expect(registry('source_server', MULTI.romId)).toBe(SERVER);

    const since = (await requests()).slice(requestsBefore);
    expect(since.filter((r) => r.path.startsWith('/api/'))).toEqual([]);
  });
});
