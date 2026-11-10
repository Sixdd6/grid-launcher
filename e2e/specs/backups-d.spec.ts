import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { FAKE_DRIVE_FILE, mountOf, packDir, readPack } from '../helpers/drives.js';
import { backupRows, show, startRecording } from '../helpers/backups.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** The games backups-seed.mjs `seedD` installs (kept in step with it). */
const ZIP = { romId: 901, title: 'Cinder Peaks' };
const NATIVE = { romId: 961, title: 'Tux Trail' };
const OUTDATED = { romId: 902, title: 'Moss and Marrow' };
const LOCAL_IDS = [ZIP.romId, NATIVE.romId, OUTDATED.romId];
/** Moss and Marrow's install-time server timestamp in the seed (the fixture has 2026-01-01). */
const OUTDATED_UPDATED_AT = '2025-06-01T00:00:00Z';

const DRIVE = 'TESTDRIVE';

/** One column of the registry row for `romId`, read with the sqlite3 CLI as other specs do. */
function registry(column: string, romId: number): string {
  return execFileSync(
    'sqlite3',
    [path.join(dataDir(), 'grid-launcher.db'), `SELECT ${column} FROM installed_games WHERE rom_id = ${romId};`],
    { encoding: 'utf-8' },
  ).trim();
}

/** Every file and folder under `dir`, relative to it, `/`-separated. */
function walk(dir: string, prefix = ''): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const rel = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
    return entry.isDirectory() ? [rel, ...walk(path.join(dir, entry.name), rel)] : [rel];
  });
}

/** The mock's live request log (the file is only written when the mock stops). */
async function requests(): Promise<{ method: string; path: string }[]> {
  return (await (await fetch(`${mockUrl()}/__e2e__/requests`)).json()) as { method: string; path: string }[];
}

/** The text of every Source row: "<name> … <tag>". */
async function sourceRowTexts(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="backup-source-row"]')).map((row) =>
      (row.textContent ?? '').replace(/\s+/g, ' ').trim(),
    ),
  );
}

/**
 * Stage `backups/backups-d`: backup three games installed on this computer
 * (an extracted zip, a Linux game with its game.json, and a multi-file game
 * that is out of date) by copying their installs, then restore one from the
 * drive. The seed gives a fresh profile with spec a's PCSX2 setup and a
 * TESTDRIVE set up for Linux.
 */
describe('backups-d: backup installed games from this computer and restore one', () => {
  let requestsBefore = 0;

  before(async () => {
    // backups-c left the mock offline.
    const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ offline: false }),
    });
    expect(res.ok).toBe(true);

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

  it('opens the Backup dialog for the three games with the checkbox ticked and their sources listed', async () => {
    await show('library');
    await $(testId('library-rail-all')).click();
    for (const id of LOCAL_IDS) {
      await $(testId(`library-card-${id}`)).waitForDisplayed({
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `Library never showed the installed rom ${id}`,
      });
    }
    await $(testId('library-select')).click();
    await expect($(testId('library-select'))).toHaveText('Done');
    for (const id of LOCAL_IDS) await $(testId(`library-card-${id}`)).click();
    await expect($(testId('library-selection-count'))).toHaveText(expect.stringContaining('3 selected'));
    await $(testId('library-selection-backup')).click();

    const dialog = $(testId('backup-dialog'));
    await dialog.waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the Backup dialog never opened' });
    await expect($(testId('backup-dialog-title'))).toHaveText('Backup 3 games');

    await $(testId('backup-source')).waitForDisplayed({
      timeout: 15_000,
      timeoutMsg: 'the Backup dialog never showed its Source section',
    });
    expect(await $(testId('backup-use-local')).isSelected()).toBe(true);
    await expect($(testId('backup-lede'))).toHaveText(expect.stringContaining('copies games installed on this computer'));

    let rows: string[] = [];
    await browser
      .waitUntil(
        async () => {
          rows = await sourceRowTexts();
          return rows.length === 3;
        },
        { timeout: 15_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the Source section never listed the three games: ${JSON.stringify(rows)}`);
      });
    const row = (title: string) => rows.find((r) => r.startsWith(title)) ?? '';
    expect(row(ZIP.title)).toMatch(/From this computer$/);
    expect(row(NATIVE.title)).toMatch(/From this computer$/);
    expect(row(OUTDATED.title)).toContain('the server has newer files');
    expect(row(OUTDATED.title)).toMatch(/Choose below$/);
    await expect($(testId('backup-local-line'))).toHaveText(
      expect.stringMatching(/^2 games from this computer \(.+\) · 1 to choose$/),
    );
  });

  it('asks which copy of the out-of-date game to backup', async () => {
    const outdated = $(testId('backup-outdated'));
    await outdated.waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the out-of-date prompt never showed' });
    await expect(outdated).toHaveText(expect.stringContaining('The server has newer files'));
    const choices = await $$(testId('backup-choice'));
    expect(choices).toHaveLength(1);
    await expect(choices[0]).toHaveText(expect.stringContaining(OUTDATED.title));

    const start = $(testId('backup-start'));
    await browser.waitUntil(async () => (await start.getText()).trim() === 'Choose for 1 game', {
      timeout: 15_000,
      timeoutMsg: `the Backup button never read "Choose for 1 game" (it reads "${await start.getText()}")`,
    });
    expect(await start.isEnabled()).toBe(false);

    const useLocal = choices[0].$('label=Use this computer’s copy');
    await useLocal.click();
    expect(await choices[0].$('input[value="local"]').isSelected()).toBe(true);

    await browser.waitUntil(async () => (await start.getText()).trim() === 'Backup 3 games', {
      timeout: 15_000,
      timeoutMsg: `the Backup button never read "Backup 3 games" (it reads "${await start.getText()}")`,
    });
    await start.waitForEnabled({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Backup never became enabled' });
    const rows = await sourceRowTexts();
    const moss = rows.find((r) => r.startsWith(OUTDATED.title)) ?? '';
    expect(moss).toContain('an older copy than the server’s');
    expect(moss).toMatch(/From this computer$/);
    await expect($(testId('backup-local-line'))).toHaveText(expect.stringMatching(/^3 games from this computer \(.+\)$/));
  });

  it('copies the three installs to the drive without downloading them', async () => {
    requestsBefore = (await requests()).length;
    await startRecording();
    await $(testId('backup-start')).click();
    await $(testId('backup-dialog')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });

    await show('downloads');
    let rows = await backupRows();
    await browser
      .waitUntil(
        async () => {
          rows = await backupRows();
          const games = rows.filter((r) => [ZIP.title, NATIVE.title, OUTDATED.title].includes(r.title));
          return games.length === 3 && rows.every((r) => r.detail.endsWith(`Written to ${DRIVE}`));
        },
        { timeout: 60_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the Backup rows never all reached "Written to ${DRIVE}": ${JSON.stringify(rows)}`);
      });
    for (const title of [ZIP.title, NATIVE.title, OUTDATED.title]) {
      expect(rows.find((r) => r.title === title)?.detail).toBe(`Copied from this computer · Written to ${DRIVE}`);
    }

    const since = (await requests()).slice(requestsBefore);
    // The emulator and the firmware still come from the server: the log
    // does cover the backup.
    expect(since.some((r) => r.path.includes('/content/'))).toBe(true);
    const gameContent = since.filter((r) => LOCAL_IDS.some((id) => r.path.startsWith(`/api/roms/${id}/content/`)));
    expect(gameContent).toEqual([]);
  });

  it('writes each game as an installed copy in parts, inside gridpack/', async () => {
    const pack = readPack(DRIVE);
    const games = pack.games;
    expect(games.map((g) => g.source.rom_id).sort()).toEqual([...LOCAL_IDS].sort());
    const layouts: Record<number, string> = { [ZIP.romId]: 'extracted', [NATIVE.romId]: 'native', [OUTDATED.romId]: 'multi' };
    for (const game of games) {
      const installed = game.installed;
      expect(installed).toBeDefined();
      expect(installed!.layout).toBe(layouts[game.source.rom_id]);
      expect(game.files).toEqual([]);
      expect(installed!.parts.length).toBeGreaterThan(0);
      for (const part of installed!.parts) {
        // Relative to gridpack/, so the drive works under any mount point.
        expect(path.isAbsolute(part.path)).toBe(false);
        expect(part.path.split('/')).not.toContain('..');
        expect(part.path.startsWith('games/')).toBe(true);
        const onDisk = path.join(packDir(DRIVE), part.path);
        expect(existsSync(onDisk)).toBe(true);
        expect(statSync(onDisk).size).toBe(part.size);
        expect(createHash('md5').update(readFileSync(onDisk)).digest('hex')).toBe(part.md5);
      }
      // The row values a restore needs are relative: nothing names this
      // computer's folders.
      expect(JSON.stringify(installed)).not.toContain(dataDir());
    }
    const byId = (id: number) => games.find((g) => g.source.rom_id === id)!.installed!;
    expect(byId(OUTDATED.romId).outdated).toBe(true);
    expect(byId(OUTDATED.romId).server_updated_at).toBe(OUTDATED_UPDATED_AT);
    expect(byId(ZIP.romId).outdated).toBe(false);
    expect(byId(NATIVE.romId).launch).toBe('game/tuxtrail.sh');

    // GRID wrote nothing outside gridpack/ (the fake drive's own probe file
    // is the test's), and no temporary file is left.
    expect(readdirSync(mountOf(DRIVE)).sort()).toEqual([FAKE_DRIVE_FILE, 'gridpack'].sort());
    expect(walk(packDir(DRIVE)).filter((rel) => rel.endsWith('.gridtmp'))).toEqual([]);
  });

  it('restores an uninstalled game from the drive, ready to play', async () => {
    const library = path.join(dataDir(), 'library');
    const oldDir = registry('multi_file_game_dir', OUTDATED.romId);
    expect(oldDir).not.toBe('');

    await show('library');
    await $(testId('library-rail-all')).click();
    const card = $(testId(`library-card-${OUTDATED.romId}`));
    await card.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await card.click();
    // Re-queried every time: Details leaves the page between steps, and a
    // kept element reference would go stale.
    const panel = () => $(testId('details-panel'));
    await panel().waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Details never opened' });
    const uninstall = $(testId('details-uninstall'));
    await uninstall.waitForEnabled({ timeout: TRANSITION_TIMEOUT });
    await uninstall.click();
    await expect(uninstall).toHaveText('Confirm uninstall');
    await uninstall.click();
    await panel().waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Details never closed after uninstalling' });
    await browser.waitUntil(() => registry('rom_id', OUTDATED.romId) === '', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the uninstalled game stayed in the registry',
    });
    expect(existsSync(oldDir)).toBe(false);

    const driveRow = $('[data-testid^="library-rail-drive-"]');
    await driveRow.waitForExist({ timeout: 10_000, timeoutMsg: 'Library never listed the backup drive' });
    await driveRow.click();
    const driveCard = $(testId(`library-drive-card-${OUTDATED.romId}`));
    await driveCard.waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: "the drive's copy never showed in Library" });
    await driveCard.click();
    await panel().waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const install = $(testId('details-install'));
    await expect(install).toHaveText('Install from drive');
    await install.click();
    await browser.waitUntil(() => registry('rom_id', OUTDATED.romId) !== '', {
      timeout: 30_000,
      timeoutMsg: 'the restored game never reached the registry',
    });
    if (await panel().isExisting()) {
      await $(testId('details-close')).click();
      await panel().waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    }

    // The row points into the library, and keeps the copy's own server
    // timestamp: the restored game is still the older version.
    const dir = registry('multi_file_game_dir', OUTDATED.romId);
    const launch = registry('extracted_path', OUTDATED.romId);
    expect(dir.startsWith(library + path.sep)).toBe(true);
    expect(launch.startsWith(dir + path.sep)).toBe(true);
    expect(path.basename(launch)).toBe('game.m3u');
    expect(existsSync(launch)).toBe(true);
    expect(readdirSync(dir).sort()).toEqual(['disc1.bin', 'disc2.bin', 'game.m3u']);
    expect(registry('server_updated_at', OUTDATED.romId)).toBe(OUTDATED_UPDATED_AT);

    await $(testId('library-rail-all')).click();
    const restored = $(testId(`library-card-${OUTDATED.romId}`));
    await restored.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await restored.click();
    await panel().waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const play = $(testId('details-play'));
    await play.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Details never offered Play' });
    await browser.waitUntil(async () => play.isEnabled(), {
      timeout: 15_000,
      timeoutMsg: 'Play never became enabled',
    });
    await $(testId('details-close')).click();
  });
});
