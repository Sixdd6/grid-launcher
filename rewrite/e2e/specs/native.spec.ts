import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  REAP_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `native`: a Windows-platform ("native") game, which grid-core
 * installs into its own home directory rather than the platform root —
 * `<library>/Windows/<Title>/{game/,prefix,game.json}` — and launches
 * through a compat tool instead of an emulator entry.
 *
 * The seed puts a `wine` stub first on the app's PATH (see native-seed.mjs
 * and wdio.conf.ts's `PATH` handling) and sets `default_compat_tool =
 * "wine"`, so `build_native_command`'s `which("wine")`
 * (grid-core/src/launch/native.rs) resolves to that stub and the launch it
 * records is the real argv the app built.
 *
 * Rom 702 exists only for the Cancel smoke: its ~300KB archive carries an
 * `e2e_throttle` on its fixture file entry (mock-romm/server.mjs), so that
 * ONE download streams slowly enough to be cancelled mid-flight while every
 * other install in this group still runs at full speed.
 *
 * Queue ids across this spec's single app instance: 1 = rom 701's install,
 * 2 = rom 702's.
 */
describe('native', () => {
  const gameDir = () => path.join(dataDir(), 'library', 'Windows', 'My Game');
  const wineArgvLog = () => path.join(dataDir(), 'wine-argv.log');

  async function openDetails(romId: number) {
    await $(testId(`game-card-${romId}`)).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId(`game-card-${romId}`)).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the details overlay never opened for rom ${romId}`,
    });
  }

  async function closeDetails() {
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
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

    await $(testId('downloads-footer')).click();
    await $(testId('downloads-drawer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads drawer never opened',
    });
    await $(testId('platform-btn-1')).click();
  });

  it('labels the primary button "Install App" for a Windows game', async () => {
    await openDetails(701);
    await expect($(testId('details-install'))).toHaveText('Install App');
  });

  it('installs the game into its own home directory with a Wine prefix', async () => {
    await $(testId('details-install')).click();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the native install never reached Completed' },
    );
    await $(testId('installed-badge-701')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 701's card",
    });

    expect(existsSync(path.join(gameDir(), 'game', 'MyGame', 'mygame.exe'))).toBe(true);
    expect(existsSync(path.join(gameDir(), 'prefix'))).toBe(true);
  });

  it('lists the extracted executable in Game Settings and saves launch parameters', async () => {
    await $(testId('details-game-settings')).click();
    await $(testId('native-settings-exe')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Game Settings dialog never listed an executable',
    });

    // The option VALUE is the candidate's absolute path; its visible label is
    // that path relative to the install dir the backend's shallowest
    // candidate implies (`installDirOf`/`candidateLabel`, details/actions.ts).
    // With one candidate that relative form is just `mygame.exe`, so the
    // assertion that actually pins WHICH file was found is on the value.
    const options = await $$(`${testId('native-settings-exe')} option`);
    expect(options.length).toBe(1);
    const value = await options[0].getValue();
    expect(value.endsWith(path.join('game', 'MyGame', 'mygame.exe'))).toBe(true);

    await $(testId('native-settings-params')).setValue('--fullscreen');
    await $(testId('native-settings-save')).click();
    await $(testId('native-settings')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the Game Settings dialog never closed after saving',
    });
  });

  it('launches through the wine stub with the executable and the saved parameters', async () => {
    await $(testId('details-play')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-playing-chip never appeared after Play',
    });

    await browser.waitUntil(
      () => {
        try {
          return readFileSync(wineArgvLog(), 'utf-8').includes('--fullscreen');
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the wine stub never recorded its argv',
      },
    );
    const argv = readFileSync(wineArgvLog(), 'utf-8').split('\n').filter(Boolean);
    expect(argv[0].endsWith(path.join('game', 'MyGame', 'mygame.exe'))).toBe(true);
    expect(argv).toContain('--fullscreen');

    await $(testId('details-stop')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: REAP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'details-playing-chip never cleared after Stop within the reaper window',
    });
  });

  it('cancels an in-flight install from the details overlay', async () => {
    await closeDetails();
    await openDetails(702);
    await $(testId('details-install')).click();

    // `details-cancel` replaces the install button while a live drawer entry
    // exists for this rom — the throttled fixture is what keeps that window
    // open long enough to click it.
    await $(testId('details-cancel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-cancel never appeared for the in-flight install',
    });
    await $(testId('details-cancel')).click();

    await browser.waitUntil(
      async () => (await $(testId('download-detail-2')).getText()) === 'Cancelled',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the cancelled install never showed the Cancelled status',
      },
    );
  });
});
