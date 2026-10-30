import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  REAP_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Upper bound for the native save panel's PCGamingWiki lookup to settle.
 * `native_save_paths` fetches the live wiki (`PCGW_API_BASE`, grid-core's
 * pcgw.rs — there is no mock for it), which is up to two sequential requests
 * on a 10s-timeout client, and it degrades a failure to an empty list rather
 * than an error. Whether this host has network access or not, the panel
 * leaves its "loading" phase within this budget.
 */
const PCGW_LOOKUP_TIMEOUT = 30_000;

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
 * Rom 702 exists only for the Cancel smoke: its ~2MB archive carries an
 * `e2e_throttle` on its fixture file entry (mock-romm/server.mjs), so that
 * ONE download streams slowly enough to be cancelled mid-flight while every
 * other install in this group still runs at full speed.
 *
 * Rom 703 is on the `Linux` platform: native too (user ruling 2026-09-08),
 * but it takes no compat tool, so its payload is a real `#!/bin/sh` script
 * that the app spawns directly (the install chmods the selected launch file
 * itself) — the `wine` stub must stay untouched for it.
 *
 * Rom 704 is on platform 3, RomM's `win` platform under the custom display
 * name "PC Fangames & Ports": native only by its slug (user bug 2026-10-10).
 *
 * Rom 705 is on the `Linux` platform with a `game.json` whose `launch`
 * section names `SideGame/launch.sh` (arguments and working dir `.`) among
 * two scripts; the automatic ranking alone would pick `sidegame.sh` (user
 * feature 2026-10-10, launch metadata in the sidecar).
 *
 * Queue ids across this spec's single app instance: 1 = rom 701's install,
 * 2 = rom 702's, 3 = rom 703's, 4 = rom 704's, 5 = rom 705's.
 */
describe('native', () => {
  const gameDir = () => path.join(dataDir(), 'library', 'games', 'Windows', 'My Game');
  const linuxGameDir = () => path.join(dataDir(), 'library', 'games', 'Linux', 'Tux Game');
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

    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });

    await showServer();
    await $(testId('platform-btn-1')).click();
  });

  it('labels the primary button "Install App" for a Windows game', async () => {
    await openDetails(701);
    await expect($(testId('details-install'))).toHaveText('Install App');
  });

  it('states no emulator launch target for a native game', async () => {
    // User ruling 2026-09-05: a Windows/Linux game runs its own executable
    // through a compat tool, so the popup renders no launch-target line at
    // all rather than "No default emulator" (details/header.ts).
    await expect($(testId('details-emulator'))).not.toExist();
    // The line beside it is still rendered, so this proves the aside itself
    // is present and only the emulator row is gone.
    await expect($(testId('details-last-played'))).toExist();
  });

  it('installs the game into its own home directory with a Wine prefix', async () => {
    await $(testId('details-install')).click();
    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the native install never reached Completed' },
    );
    await $(`${testId('server-view')} ${testId('installed-badge-701')}`).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 701's card",
    });

    expect(existsSync(path.join(gameDir(), 'game', 'MyGame', 'mygame.exe'))).toBe(true);
    expect(existsSync(path.join(gameDir(), 'prefix'))).toBe(true);
  });

  it('lists the extracted executable in Game Settings and saves launch parameters', async () => {
    await showServer();
    await $(testId('details-game-settings')).click();
    await $(testId('native-settings-exe')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Game Settings dialog never listed an executable',
    });

    // The option VALUE is the candidate's absolute path; its visible label is
    // that path relative to the install directory the BACKEND reports
    // (`NativeGameSettings.install_dir`, falling back to `installDirOf` for
    // a row written before that field existed — NativeSettings.svelte —
    // then rendered by `candidateLabel`, details/actions.ts). The label is
    // therefore a display concern; the assertion that pins WHICH file was
    // found is on the option's value, because the value is what identifies
    // the file.
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

  /** Opens the Saves tab's native save panel for the currently open game. */
  async function openNativeSavePanel() {
    // The Saves tab shows the list itself; there is no toggle to open it.
    await $(testId('details-tab-saves')).click();
    await $(testId('cloud-native-status')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the native save-location section never rendered',
    });
    // `cloud-native-fetching` exists only while the PCGamingWiki lookup is
    // in flight, so its disappearance IS the "loaded" phase. Every status
    // and row assertion below reads the settled list, not the lookup's
    // placeholder text.
    await $(testId('cloud-native-fetching')).waitForExist({
      timeout: PCGW_LOOKUP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the PCGamingWiki lookup never settled',
    });
  }

  /**
   * The `[native_removed_save_paths]` table of the on-disk config, sliced
   * out so a match cannot come from the `native_manual_save_paths` table
   * that holds the same key.
   */
  function removedSavePathsTable(): string {
    const text = readFileSync(configPath(), 'utf-8');
    const after = text.split('[native_removed_save_paths]')[1] ?? '';
    return after.split('\n[')[0];
  }

  it('adds, lists and removes a manual native save location', async () => {
    await openNativeSavePanel();

    const manual = path.join(dataDir(), 'manual-saves');
    await $(testId('cloud-native-path-input')).setValue(manual);
    await $(testId('cloud-native-path-add')).click();

    const row = $(`[data-testid="cloud-native-path-manual-${manual}"]`);
    await row.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the manual save location never appeared in the list',
    });
    // The row's tooltip is the path this host would really read
    // (`native_path_entries`' expanded form), so it is never blank.
    expect(await row.getAttribute('title')).not.toBe('');

    // The Browse button opens a NATIVE folder dialog, which no WebDriver
    // session can dismiss — assert it exists, never click it.
    await expect($(testId('cloud-native-path-browse'))).toExist();

    // "N save location(s) configured." — the count is asserted as a regex
    // because a live PCGamingWiki answer would legitimately raise it.
    const status = await $(testId('cloud-native-status')).getText();
    expect(status).toMatch(/^\d+ save location\(s\) configured\.$/);
    expect(Number(status.split(' ')[0])).toBeGreaterThanOrEqual(1);

    await $(`[data-testid="cloud-native-path-remove-${manual}"]`).click();
    await row.waitForExist({
      timeout: PCGW_LOOKUP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the manual save location never disappeared after Remove',
    });
  });

  it('persists the removal across a popup reopen', async () => {
    const manual = path.join(dataDir(), 'manual-saves');

    // The suppression list is what makes a removal survive: it is written to
    // the config, not just to the open panel's state.
    await browser.waitUntil(() => removedSavePathsTable().includes(manual), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the removal never reached native_removed_save_paths in the config',
    });

    await closeDetails();
    await openDetails(701);
    await openNativeSavePanel();
    await expect($(`[data-testid="cloud-native-path-manual-${manual}"]`)).not.toExist();
    // Reconciled against a real run (task 10, step 4): the fixture title
    // "My Game" has no PCGamingWiki article, so the settled list is empty
    // with or without network and the status line is the "none found" one.
    // The removed row's absence above is the real subject either way.
    await expect($(testId('cloud-native-status'))).toHaveText(
      'No save locations found on PCGamingWiki.',
    );
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

    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-2')).getText()) === 'Cancelled',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the cancelled install never showed the Cancelled status',
      },
    );
  });

  // User ruling 2026-09-08: a Linux-PLATFORM game is native exactly like a
  // Windows one — same install layout, same "Install App" label, no install
  // block reason — but it takes NO compat tool, so Play spawns its own
  // executable and the `wine` stub is never reached.
  it('installs a Linux-platform game the same way', async () => {
    await showServer();
    await $(testId('platform-btn-2')).click();
    await openDetails(703);
    await expect($(testId('details-install'))).toHaveText('Install App');

    await $(testId('details-install')).click();
    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-3')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the Linux install never reached Completed' },
    );
    await showServer();
    await $(`${testId('server-view')} ${testId('installed-badge-703')}`).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 703's card",
    });

    expect(existsSync(path.join(linuxGameDir(), 'game', 'TuxGame', 'tuxgame.sh'))).toBe(true);
  });

  it('launches the Linux game directly, with no wine stub in between', async () => {
    const before = existsSync(wineArgvLog()) ? readFileSync(wineArgvLog(), 'utf-8') : '';

    await $(testId('details-play')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-playing-chip never appeared after Play on the Linux game',
    });

    // The spawned process IS tuxgame.sh: if a compat tool had been applied,
    // the seeded `wine` stub would have appended its argv to this log.
    const after = existsSync(wineArgvLog()) ? readFileSync(wineArgvLog(), 'utf-8') : '';
    expect(after).toBe(before);
    expect(after).not.toContain('tuxgame.sh');

    await $(testId('details-stop')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: REAP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'details-playing-chip never cleared after Stop on the Linux game',
    });
  });

  // User bug 2026-10-10: platform 3 is RomM's `win` platform under a custom
  // display name, "PC Fangames & Ports" (fs_slug `win-fangames-ports`). The
  // display name is not a Windows name, so only the platform SLUG makes rom
  // 704 native: no emulator chip, "Install App", no emulator launch target,
  // the native install layout, and Game Settings afterwards. Queue id 4.
  it('treats a custom-named Windows platform as native by its slug', async () => {
    await closeDetails();
    await $(testId('platform-btn-3')).click();
    await expect($(testId('server-emulator-chip'))).not.toExist();

    await openDetails(704);
    await expect($(testId('details-install'))).toHaveText('Install App');
    await expect($(testId('details-emulator'))).not.toExist();

    await $(testId('details-install')).click();
    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-4')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the custom-named Windows install never reached Completed' },
    );
    await showServer();
    await $(`${testId('server-view')} ${testId('installed-badge-704')}`).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 704's card",
    });

    const fanGameDir = path.join(dataDir(), 'library', 'games', 'PC Fangames & Ports', 'Fan Port');
    expect(existsSync(path.join(fanGameDir, 'game', 'MyGame', 'mygame.exe'))).toBe(true);
    expect(existsSync(path.join(fanGameDir, 'prefix'))).toBe(true);

    await $(testId('details-game-settings')).click();
    await $(testId('native-settings-exe')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Game Settings never listed an executable for the custom-named Windows game',
    });
    const options = await $$(`${testId('native-settings-exe')} option`);
    expect(options.length).toBe(1);
    expect((await options[0].getValue()).endsWith(path.join('game', 'MyGame', 'mygame.exe'))).toBe(true);
  });

  // User feature 2026-10-10: the `game.json` sidecar's `launch` section
  // pins the launch file at install. Rom 705's archive holds two scripts;
  // the ranking alone would choose `sidegame.sh`, the sidecar names
  // `launch.sh` with two arguments (one with a space) and working dir "."
  // (the game folder, not the script's own `SideGame/`). Queue id 5.
  it('launches the file, arguments and working dir the game.json launch section names', async () => {
    await $(testId('native-settings-close')).click();
    await $(testId('native-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await closeDetails();
    await $(testId('platform-btn-2')).click();
    await openDetails(705);

    await $(testId('details-install')).click();
    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-5')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the sidecar-launch install never reached Completed' },
    );
    await showServer();
    await $(`${testId('server-view')} ${testId('installed-badge-705')}`).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: "the installed badge never appeared on rom 705's card",
    });

    const sideGameDir = path.join(dataDir(), 'library', 'games', 'Linux', 'Side Game');
    const installDir = path.join(sideGameDir, 'game');
    const launchScript = path.join(installDir, 'SideGame', 'launch.sh');
    expect(existsSync(launchScript)).toBe(true);
    expect(existsSync(path.join(installDir, 'SideGame', 'sidegame.sh'))).toBe(true);

    // Game Settings shows the sidecar's file, marked as set by game.json.
    await $(testId('details-game-settings')).click();
    await $(testId('native-settings-exe')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Game Settings never listed an executable for the sidecar game',
    });
    expect(await $(testId('native-settings-exe')).getValue()).toBe(launchScript);
    await expect($(testId('native-launch-source'))).toHaveText('Set by game.json');
    expect(await $(testId('native-settings-params')).getValue()).toBe('--from-sidecar "two words"');

    // The reload action re-reads the same sidecar and keeps the pin.
    await $(testId('native-launch-reload')).click();
    await expect($(testId('native-launch-reload-result'))).toHaveText('Launch file set from game.json.');
    await expect($(testId('native-launch-source'))).toHaveText('Set by game.json');

    await $(testId('native-settings-close')).click();
    await $(testId('native-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });

    const argvLog = path.join(installDir, 'SideGame', 'launch-argv.log');
    await $(testId('details-play')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-playing-chip never appeared after Play on the sidecar game',
    });
    await browser.waitUntil(() => existsSync(argvLog), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'launch.sh never ran: the sidecar launch file was not the one started',
    });
    await browser.waitUntil(() => readFileSync(argvLog, 'utf-8').split('\n').filter(Boolean).length >= 4, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'launch.sh never finished writing its argv log',
    });
    // Line 1: the working dir (the game folder, from working_dir "."),
    // line 2: the started file itself, then each argument.
    const lines = readFileSync(argvLog, 'utf-8').split('\n').filter(Boolean);
    expect(lines).toEqual([installDir, launchScript, '--from-sidecar', 'two words']);

    await $(testId('details-stop')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: REAP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'details-playing-chip never cleared after Stop on the sidecar game',
    });
  });
});
