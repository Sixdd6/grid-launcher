import { existsSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `install`, part A: connect, set the library path, install rom 101
 * (a single-file SNES zip whose one entry is `game.sfc` — see
 * mock-romm/server.mjs's content fixtures), and assert it lands on disk.
 *
 * Part B relaunches the same binary against the same data dir and asserts
 * the `installed` badge survives the restart, then runs the uninstall flow.
 * Split for the same reason as `connect-restore`: the embedded WebDriver
 * provider cannot restart the app inside one `wdio run`.
 */
describe('install (a): connect, set library path, install', () => {
  // The five views no longer stack (design §3): one pill click swaps which
  // root is displayed, so a spec has to be on the right view before it reads
  // text from it or clicks anything inside it.
  async function showDownloads() {
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  }

  async function showServer() {
    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never came back',
    });
  }

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after connecting',
    });

    // Prove the Downloads pill reaches its view once, up front; the tests
    // below switch back and forth as they need it.
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  });

  it('shows the library-path banner when unset, and hides it once a path is saved', async () => {
    await showServer();
    const banner = $(testId('library-path-banner'));
    await expect(banner).toExist();

    const libraryPath = path.join(dataDir(), 'library');
    await $(testId('library-path-input')).setValue(libraryPath);
    await $(testId('library-path-save')).click();

    await banner.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the library-path banner never hid after saving a path',
    });
  });

  it('installs a game from the details overlay through to the installed badge', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();

    const panel = $(testId('details-panel'));
    await panel.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 101',
    });
    await $(testId('details-install')).click();

    await showDownloads();

    // A download row appears for the new entry (id 1: the first admitted
    // job in this fresh app instance's queue) — even for a fast install
    // this proves the row existed, not just that installation eventually
    // succeeded.
    const row = $('[data-testid^="download-row-"]');
    await row.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no download-row appeared after clicking install',
    });

    const detail = $('[data-testid^="download-detail-"]');
    await browser.waitUntil(async () => (await detail.getText()).startsWith('Completed'), {
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the download never reached Completed',
    });

    await $(testId('installed-badge-101')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the installed badge never appeared on rom 101\'s card',
    });
  });

  it('extracts the zip\'s game.sfc under the temp library dir', () => {
    const extracted = path.join(
      dataDir(),
      'library',
      'Super Nintendo Entertainment System',
      'Super Mario World',
      'game.sfc',
    );
    expect(existsSync(extracted)).toBe(true);
  });
});
