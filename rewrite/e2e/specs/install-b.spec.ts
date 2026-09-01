import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const extractedDir = () =>
  path.join(dataDir(), 'library', 'Super Nintendo Entertainment System', 'Super Mario World');

/**
 * Stage `install`, part B: a second launch of the same binary against the
 * data directory part A left behind. Nothing here reconnects or re-installs
 * — the session restores from config.toml + the keyring (like
 * connect-restore-b), and rom 101's `installed` badge must already be there
 * from part A's install.
 */
describe('install (b): relaunch, badge persists, then uninstall', () => {
  before(async () => {
    await $(testId('platform-btn-1')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the library never appeared — the stored session was not restored',
    });
  });

  it('keeps the installed badge across a restart', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('installed-badge-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the installed badge did not survive the restart',
    });
    expect(existsSync(path.join(extractedDir(), 'game.sfc'))).toBe(true);
  });

  it('uninstalls via the details two-click, removing the badge and the files', async () => {
    await $(testId('game-card-101')).click();
    const panel = $(testId('details-panel'));
    await panel.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for the installed game',
    });

    const uninstallBtn = $(testId('details-uninstall'));
    await uninstallBtn.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Uninstall button never appeared for an installed game',
    });

    // First click only arms the confirmation.
    await uninstallBtn.click();
    await expect(uninstallBtn).toHaveText('Confirm uninstall');
    await expect(panel).toExist(); // still open — nothing happened yet

    // Second click actually uninstalls, which closes the overlay on success.
    await uninstallBtn.click();
    await panel.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the details overlay never closed after uninstalling',
    });

    await $(testId('installed-badge-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the installed badge was still there after uninstalling',
    });
    expect(existsSync(extractedDir())).toBe(false);
  });

  it('never writes the fixture token into grid-launcher.db', () => {
    const dbPath = path.join(dataDir(), 'grid-launcher.db');
    const bytes = readFileSync(dbPath);
    expect(bytes.includes(FIXTURE_TOKEN)).toBe(false);
  });
});
