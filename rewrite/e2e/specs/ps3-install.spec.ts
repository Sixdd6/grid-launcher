import { execFileSync } from 'node:child_process';
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
 * Stage `ps3-install`: a PlayStation 3 install with NO emulator configured.
 *
 * That is the point of the group. `ps3_roots_from_config`
 * (grid-core/src/library/mod.rs) resolves the RPCS3 VFS from the default PS3
 * emulator's own `vfs.yml` when there is one, and otherwise falls back to
 * `<library>/PlayStation 3/.vfs/dev_hdd0` — the branch nothing else covers.
 * The seed writes no `config.toml` at all, so this spec sets the library
 * path through the UI exactly as install-a.spec.ts does.
 *
 * Rom 401's archive (`mock-romm/server.mjs`'s `ps3ZipBytes`) holds
 * `BLUS30336/PS3_GAME/{USRDIR/EBOOT.BIN,PARAM.SFO}` — the shape
 * `specials::ps3::classify` calls a top-level game-id directory and routes
 * whole into `dev_hdd0/game/<GAMEID>/`.
 */
describe('ps3-install', () => {
  const library = () => path.join(dataDir(), 'library');
  const devHdd0 = () => path.join(library(), 'PlayStation 3', '.vfs', 'dev_hdd0');

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

    await $(testId('library-path-input')).setValue(library());
    await $(testId('library-path-save')).click();
    await $(testId('library-path-banner')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the library-path banner never hid after saving a path',
    });

    // Prove the Downloads pill reaches its view once, up front; the test
    // below switches back and forth as it needs it.
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  });

  it('installs the PS3 game through to a Completed download row', async () => {
    await showServer();
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-401')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-401')).click();

    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 401',
    });
    await $(testId('details-install')).click();

    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Completed'),
      {
        timeout: INSTALL_TIMEOUT,
        timeoutMsg: 'the PS3 install never reached Completed',
      },
    );

    await $(testId('installed-badge-401')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 401's card",
    });
  });

  it('routes the game tree into the library VFS and clears the staging dir', () => {
    expect(
      existsSync(path.join(devHdd0(), 'game', 'BLUS30336', 'PS3_GAME', 'USRDIR', 'EBOOT.BIN')),
    ).toBe(true);
    // `specials::ps3::route` moves the tree and then removes what it
    // extracted; the downloaded archive goes too.
    expect(existsSync(path.join(library(), 'PlayStation 3', 'game'))).toBe(false);
    expect(existsSync(path.join(library(), 'PlayStation 3', 'game.zip'))).toBe(false);
  });

  it("records the detected game id on the registry row", () => {
    // Read with the sqlite3 CLI the seeds already depend on, rather than
    // adding a node sqlite dependency to the e2e package.
    const out = execFileSync(
      'sqlite3',
      [
        path.join(dataDir(), 'grid-launcher.db'),
        'SELECT ps3_game_id FROM installed_games WHERE rom_id = 401;',
      ],
      { encoding: 'utf-8' },
    );
    expect(out.trim()).toBe('BLUS30336');
  });
});
