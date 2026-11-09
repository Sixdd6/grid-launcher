import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { recordedDetails, show, startRecording, waitForRecordedToast } from '../helpers/backups.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** One column of the registry row for `title`, read with the sqlite3 CLI as other specs do. */
function registry(column: string, title: string): string {
  return execFileSync(
    'sqlite3',
    [path.join(dataDir(), 'grid-launcher.db'), `SELECT ${column} FROM installed_games WHERE title = '${title}';`],
    { encoding: 'utf-8' },
  ).trim();
}

function configText(): string {
  try {
    return readFileSync(configPath(), 'utf-8');
  } catch {
    return '';
  }
}

/**
 * Stage `backups/backups-c`: GRID with no server. A fresh profile (the seed
 * removes spec a and b's config, registry and keyring item) and a seeded
 * TESTDRIVE whose pack "Travel kit" holds one PlayStation 2 game (rom 777
 * on a server this computer never saw), PCSX2 for Linux and a PS2 BIOS. The
 * mock RomM is switched off, so nothing here may need it.
 */
describe('backups-c: use without a server and install from a drive', () => {
  const DRIVE = 'TESTDRIVE';
  const PACK = 'Travel kit';
  const ROM = 777;

  before(async () => {
    const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ offline: true }),
    });
    expect(res.ok).toBe(true);

    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'a fresh profile never showed the Connect form',
    });
  });

  it('opens Library on the drive after "Use without a server"', async () => {
    const libraryField = $(testId('connect-library-path'));
    await libraryField.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Connect never asked for a library folder' });
    await libraryField.setValue(path.join(dataDir(), 'library'));
    const offline = $(testId('connect-use-offline'));
    await offline.waitForEnabled({ timeout: TRANSITION_TIMEOUT, timeoutMsg: '"Use without a server" never became enabled' });
    await offline.click();

    await $(testId('library-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'offline mode did not open on Library',
    });
    await expect($(testId('session-chip'))).toHaveText('Offline · no server');
    // Library opens on the first connected backup drive.
    const driveRow = $('[data-testid^="library-rail-drive-"]');
    await driveRow.waitForExist({ timeout: 10_000, timeoutMsg: 'Library never listed the backup drive' });
    await browser.waitUntil(async () => (await driveRow.getAttribute('aria-current')) === 'page', {
      timeout: 10_000,
      timeoutMsg: 'Library did not open on the backup drive',
    });
    await expect($(testId('library-title'))).toHaveText(PACK);
    await expect($(testId('library-count'))).toHaveText(expect.stringContaining(`1 game`));
    await expect($(testId('library-count'))).toHaveText(expect.stringContaining(`on ${DRIVE}`));
    // The chip's menu has one way out: Connect to a server….
    await $(testId('session-chip')).click();
    await expect($(testId('session-connect'))).toBeDisplayed();
    await $(testId('session-chip')).click();
  });

  it('shows the offline panel on Server', async () => {
    await show('server');
    await expect($(testId('server-offline-mode'))).toBeDisplayed();
    await expect($(testId('server-connect'))).toBeDisplayed();
    await show('library');
  });

  it('installs the game from the drive with its emulator, offline', async () => {
    const card = $(testId(`library-drive-card-${ROM}`));
    await card.waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: "the drive's game never showed in Library" });
    await card.click();
    await $(testId('details-panel')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-from-drive'))).toHaveText(`From ${PACK}`);
    const install = $(testId('details-install'));
    await expect(install).toHaveText('Install from drive');
    await startRecording();
    await install.click();

    // PCSX2 comes from the drive too: PlayStation 2 has no emulator here.
    await waitForRecordedToast(new RegExp(`^PCSX2 \\(Playstation 2\\) from ${PACK} added to download queue$`), 15_000);
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });

    await browser.waitUntil(() => registry('rom_id', 'Travel Game') !== '', {
      timeout: 30_000,
      timeoutMsg: 'the game never reached the registry',
    });
    // The copy read the drive, not the network.
    const lines = await recordedDetails();
    expect(lines.some((l) => l.includes(`Copying from ${DRIVE}`))).toBe(true);
    // The pack's server id is kept: offline, nothing can contradict it.
    expect(registry('rom_id', 'Travel Game')).toBe(String(ROM));
    expect(registry('source_server', 'Travel Game')).toBe('http://romm.e2e.invalid');
  });

  it("makes the drive's emulator the system's default", async () => {
    await browser.waitUntil(() => /"PlayStation 2"\s*=\s*"PCSX2 \(Playstation 2\)"/.test(configText()), {
      timeout: 30_000,
      timeoutMsg: `PCSX2 from the drive never became PlayStation 2's default:\n${configText()}`,
    });
    expect(configText()).toContain('romm:ps2/pcsx2 (playstation 2)');
  });

  it('offers Play, and disables Backup to drive with its reason', async () => {
    const platformRow = $('[data-testid^="library-rail-platform-"]');
    await platformRow.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Library never listed the installed platform' });
    await platformRow.click();
    const card = $(testId(`library-card-${ROM}`));
    await card.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await card.click();
    await $(testId('details-panel')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });

    const play = $(testId('details-play'));
    await play.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Details never offered Play' });
    await browser.waitUntil(async () => play.isEnabled(), {
      timeout: 15_000,
      timeoutMsg: 'Play never became enabled',
    });
    const backup = $(testId('details-backup'));
    expect(await backup.isEnabled()).toBe(false);
    // details/actions.ts BACKUP_NEEDS_SERVER: the hover title, and the same
    // words as the visible line under the button.
    expect(await backup.getAttribute('title')).toBe('Connect to a server to backup games');
    await expect($('#details-backup-offline')).toHaveText('Connect to a server to backup games.');
    await $(testId('details-close')).click();
  });
});
