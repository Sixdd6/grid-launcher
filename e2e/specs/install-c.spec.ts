import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  INSTALL_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const SNES = 'Super Nintendo Entertainment System';
const oldRoot = () => path.join(dataDir(), 'library');
const newRoot = () => path.join(dataDir(), 'library-new');
const gameFile = (root: string) => path.join(root, 'games', SNES, 'Super Mario World', 'game.sfc');

/**
 * Stage `install`, part C (Q2, Settings › Library): a third launch against
 * the data directory parts A and B left behind (session restored, rom 101
 * uninstalled). Reinstall rom 101 into the first library folder, change the
 * folder in Settings › Library with Start fresh › Leave on disk, and check
 * that the old row is gone, its files stay, the config records the old
 * folder, and a new install lands under the new folder.
 */
describe('install (c): change the library folder, start fresh, leave files', () => {
  async function showServer() {
    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never came back',
    });
  }

  async function installRom101() {
    await showServer();
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();
    const panel = $(testId('details-panel'));
    await panel.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 101',
    });
    await $(testId('details-install')).click();
    await $(`${testId('server-view')} ${testId('installed-badge-101')}`).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 101's card",
    });
    await $(testId('details-close')).click();
    await panel.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the details overlay never closed',
    });
  }

  before(async () => {
    await $(testId('platform-btn-1')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the library never appeared — the stored session was not restored',
    });
  });

  it('reinstalls rom 101 under the first library folder', async () => {
    await installRom101();
    expect(existsSync(gameFile(oldRoot()))).toBe(true);
  });

  it('shows the current folder in Settings › Library', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('settings-nav-library')).click();
    await $(testId('settings-library-pane')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-path-value'))).toHaveText(oldRoot());
  });

  it('refuses a folder inside the current one', async () => {
    await $(testId('library-path-change')).click();
    await $(testId('library-change-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('library-new-path-input')).setValue(path.join(oldRoot(), 'inner'));
    await $(testId('library-change-refusal')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'a folder inside the library was not refused',
    });
    await expect($(testId('library-change-refusal'))).toHaveText(
      'The new folder cannot be inside the current library folder.',
    );
    await expect($(testId('library-change-fresh'))).toBeDisabled();
    // Move existing files is phase 9b: shown, not yet available.
    await expect($(testId('library-change-move'))).toBeDisabled();
  });

  it('starts fresh and leaves the old files on disk', async () => {
    await $(testId('library-new-path-input')).setValue(newRoot());
    await $(testId('library-change-fresh')).waitForEnabled({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Start fresh never became available for a valid new folder',
    });
    await $(testId('library-change-fresh')).click();
    await $(testId('library-fresh-keep')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-fresh-delete'))).toBeDisplayed();
    await $(testId('library-fresh-keep')).click();

    await $(testId('library-change-dialog')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the change dialog never closed after Leave on disk',
    });
    await expect($(testId('library-path-value'))).toHaveText(newRoot());
    await expect($(testId('library-change-notice'))).toHaveText(
      'Library folder changed. 1 game left the library; their files stay on disk.',
    );

    // The files stay; the config records the new root and the old one.
    expect(existsSync(gameFile(oldRoot()))).toBe(true);
    const config = readFileSync(configPath(), 'utf8');
    expect(config).toContain(`library_path = ${JSON.stringify(newRoot())}`);
    expect(config).toContain('former_library_paths');
    expect(config).toContain(JSON.stringify(oldRoot()));
  });

  it('drops the old row from the library', async () => {
    await showServer();
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(`${testId('server-view')} ${testId('installed-badge-101')}`).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'rom 101 still shows as installed after Start fresh',
    });
  });

  it('installs into the new folder', async () => {
    await installRom101();
    expect(existsSync(gameFile(newRoot()))).toBe(true);
    // The old copy is untouched by the new install.
    expect(existsSync(gameFile(oldRoot()))).toBe(true);
  });
});
