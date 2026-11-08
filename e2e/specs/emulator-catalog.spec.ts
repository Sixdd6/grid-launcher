import { existsSync, readFileSync, realpathSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  forgeUrl,
  mockUrl,
  REAP_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';
import { editEmulator, findLine, groups, openRow, openSystems, pick, removeEmulator } from '../helpers/systems.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const here = path.dirname(fileURLToPath(import.meta.url));

/** Where mock-forge.mjs appends its request log (one JSON object per line). */
const forgeRequestLog = path.join(here, '..', 'last-run-forge-requests.log');

/**
 * Upper bound for one catalog install: resolve (one forge request), download
 * (a ~150-byte stub), extract, select the executable and write config. The
 * mock forge is unthrottled, so this is generous — it exists to fail fast,
 * not to allow for slow I/O.
 */
const EMULATOR_INSTALL_TIMEOUT = 15_000;

/**
 * Stage `emulator-catalog`: installing emulators from the embedded
 * autoprofile catalog through Settings › Systems' pickers (a Download pick
 * installs the emulator and makes it the system's), end to end, against
 * `mock-romm/mock-forge.mjs`.
 *
 * The app is built with the `e2e` cargo feature, so
 * `GRID_LAUNCHER_E2E_FORGE_BASE` (exported by e2e.sh as E2E_FORGE_URL and
 * forwarded by wdio.conf.ts) redirects every forge request to that mock at
 * request time (grid-core launch/forge.rs `effective_url`). Nothing else
 * changes: the app still resolves the real `https://api.github.com/...` and
 * `https://redream.io/download` URLs, which is what makes the catalog's
 * `download_url_regex` scrape genuine.
 *
 * Two profiles are covered, one per provider shape:
 * - `PCSX2 (Playstation 2)` — a `github-release` source whose linux
 *   `asset_patterns` glob matches the mock's AppImage asset. An AppImage is
 *   never extracted: the downloaded file itself becomes the emulator, kept
 *   in `<library>/emulators/PCSX2 (Playstation 2)/`.
 * - `Redream (Sega Dreamcast)` — a `direct` source, resolved by scraping the
 *   mock's HTML download page with the catalog's own regex, then extracted
 *   from a real tar.gz.
 *
 * `rewrite/e2e/seed/emulator-catalog-seed.mjs` pre-seeds the library path
 * and one installed game (rom 401, "Gran Turismo 3" on "Sony PlayStation 2",
 * from e2e/fixtures-emulator-catalog) so the freshly installed PCSX2 can be
 * made that platform's default and actually launched.
 *
 * The fixture's platform list carries a second platform, "Nintendo GameCube"
 * (id 2), purely as the negative case for the system pickers: PCSX2's
 * `platform_keywords` (["playstation 2", "ps2"]) do not match it, so
 * `compatible_emulators` must leave PCSX2 out of that system's Installed
 * group. A third, "Sega Dreamcast" (id 3, no games), is the system whose
 * picker offers Redream.
 *
 * The group also covers post-install autoconfig (doc 05 milestone-5
 * deviations, D1): right after the PCSX2 install lands, `sync_new_emulator`
 * runs against the freshly installed entry, which for PCSX2 means
 * `pcsx2::ensure_settings` (grid-core `autoconfig/pcsx2.rs`) writes the
 * managed keys into `inis/PCSX2.ini` under the AppImage's data root,
 * `<AppImage dir>/PCSX2` (grid-core `autoconfig/paths.rs`
 * `pcsx2_data_root`). The AppImage build ignores a `portable.ini`, so none
 * is written and no user-data link lands beside the file.
 */
describe('emulator-catalog', () => {
  const PLATFORM = 'Sony PlayStation 2';
  const PCSX2_NAME = 'PCSX2 (Playstation 2)';
  const PCSX2_SOURCE = 'PCSX2/pcsx2';
  const PCSX2_ASSET = 'pcsx2-v9.9-e2e-linux-appimage-x64-Qt.AppImage';
  const REDREAM_NAME = 'Redream (Sega Dreamcast)';
  const REDREAM_SOURCE = 'inolen/redream';

  const romPath = () =>
    path.join(dataDir(), 'library', 'games', PLATFORM, 'Gran Turismo 3', 'game.iso');
  const emulatorsDir = () => path.join(dataDir(), 'library', 'emulators');
  /** Layout v1: the install DIRECTORY is the profile name, untagged; only the
   *  downloaded FILE inside still carries the tag. */
  const pcsx2Path = () => path.join(emulatorsDir(), PCSX2_NAME, PCSX2_ASSET);
  /** D1: the sync runs right after install, so this is the AppImage's parent. */
  const pcsx2Dir = () => path.dirname(pcsx2Path());
  /**
   * PCSX2's AppImage roots every data directory at `<AppImage dir>/PCSX2`
   * (grid-core `autoconfig/paths.rs` `pcsx2_data_root`), so the managed ini
   * and the user-data links live here, not beside the file.
   */
  const pcsx2DataDir = () => path.join(pcsx2Dir(), 'PCSX2');
  /**
   * The tar.gz member is the bare `redream` the real tarball ships. Picking
   * it exercises `launchable_installed_file` (grid-core
   * launch/emu_install.rs): on unix an extracted file with no `.` in its
   * name and its executable bit set is launchable, alongside the reference's
   * .exe/.bat/.cmd/.ps1/.sh/.AppImage suffix set.
   */
  const redreamPath = () => path.join(emulatorsDir(), REDREAM_NAME, 'redream');

  const argvFile = (): string => {
    const value = process.env.GRID_E2E_ARGV_FILE;
    if (!value) {
      throw new Error('GRID_E2E_ARGV_FILE is not set — run this through scripts/e2e.sh');
    }
    return value;
  };

  const readForgeLog = (): string => (existsSync(forgeRequestLog) ? readFileSync(forgeRequestLog, 'utf-8') : '');

  async function waitForConfigLine(line: string) {
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(configPath(), 'utf-8').includes(line);
        } catch {
          return false;
        }
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `config.toml never got: ${line}` },
    );
  }

  async function closeSystems() {
    await $(testId('nav-server')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the settings view never went away',
    });
  }

  /** The picker's option values in one of its groups (`[]` when absent). */
  async function group(slug: string, label: 'Installed' | 'Download'): Promise<string[]> {
    return ((await groups(slug))[label] ?? []).map((o) => o.split('=')[0]);
  }

  /** Reads the open settings sheet's Program and Launch options, then closes it. */
  async function sheetFields(name: string): Promise<{ path: string; args: string }> {
    await editEmulator(await findLine(name));
    const fields = {
      path: String(await $(testId('emu-form-path')).getValue()),
      args: String(await $(testId('emu-form-args')).getValue()),
    };
    await $(testId('emu-form-cancel')).click();
    await $(testId('systems-sheet')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    return fields;
  }

  /**
   * Waits for one Downloads-view row to reach `Completed`.
   *
   * The views no longer stack (design §3), so this switches to the
   * Downloads view to read the row and hands Settings › Systems back to the
   * caller, which is where every call site is working.
   */
  async function waitForCompleted(entryId: number) {
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
    await $(testId(`download-row-${entryId}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `no downloads row appeared for emulator install ${entryId}`,
    });
    await browser.waitUntil(
      async () => (await $(testId(`download-detail-${entryId}`)).getText()).startsWith('Completed'),
      {
        timeout: EMULATOR_INSTALL_TIMEOUT,
        timeoutMsg: `emulator install ${entryId} never completed`,
      },
    );
    await openSystems();
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

    // Prove the Downloads pill reaches its view once, up front; every
    // install below is asserted through `waitForCompleted`, which switches
    // to that view and back to Settings › Systems.
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  });

  it('offers each catalog emulator in its system’s picker, recommended first', async () => {
    await openSystems();
    const ps2 = await groups('ps2');
    expect(ps2.Installed).toBeUndefined();
    expect(ps2.Download[0]).toBe(`install:${PCSX2_SOURCE}=Install PCSX2 (recommended)`);
    expect(await group('dc', 'Download')).toContain(`install:${REDREAM_SOURCE}`);
    // PCSX2 plays PlayStation 2 only.
    expect(await group('gc', 'Download')).not.toContain(`install:${PCSX2_SOURCE}`);
  });

  it('installs PCSX2 from the PS2 picker and makes it PS2’s emulator', async () => {
    await pick('ps2', `install:${PCSX2_SOURCE}`);
    await waitForCompleted(1);

    // The page re-reads itself when an emulator job reaches a terminal
    // status, so the picker moves PCSX2 from Download to Installed in place.
    await browser.waitUntil(async () => (await group('ps2', 'Installed')).includes(`use:${PCSX2_NAME}`), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the PS2 picker never listed PCSX2 under Installed',
    });
    expect(await group('ps2', 'Download')).not.toContain(`install:${PCSX2_SOURCE}`);

    // The AppImage is kept as-is (never extracted), under an install
    // directory named for the profile (layout v1 drops the tag from the
    // directory), and the config entry carries the profile's args verbatim.
    expect(existsSync(pcsx2Path())).toBe(true);
    await waitForConfigLine(pcsx2Path());
    // A picker install makes it the system's emulator once it lands.
    await waitForConfigLine(`"${PLATFORM}" = "${PCSX2_NAME}"`);
    await browser.waitUntil(async () => (await $(testId('sys-status-ps2')).getAttribute('data-status')) === 'ready', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'PS2 never showed Ready after the install',
    });

    // GameCube is offered PCSX2 nowhere: nothing installed plays it, so it
    // still has no emulator.
    expect(await group('gc', 'Installed')).toEqual([]);
    expect(JSON.stringify(await groups('gc'))).not.toContain(PCSX2_NAME);
    await expect($(testId('sys-status-gc'))).toHaveAttribute('data-status', 'none');

    const fields = await sheetFields(PCSX2_NAME);
    expect(fields.path).toBe(pcsx2Path());
    expect(fields.args).toBe('-portable -fullscreen -batch "%rom%"');
  });

  it('autoconfigures the freshly installed PCSX2 (managed PCSX2.ini keys under PCSX2/)', async () => {
    await browser.waitUntil(() => existsSync(path.join(pcsx2DataDir(), 'inis', 'PCSX2.ini')), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'autoconfig never created PCSX2 inis/PCSX2.ini under the data root',
    });

    // The AppImage build reads no `portable.ini`, so the writer leaves none
    // beside the file, and every user-data link sits under the data root.
    expect(existsSync(path.join(pcsx2Dir(), 'portable.ini'))).toBe(false);
    expect(existsSync(path.join(pcsx2Dir(), 'inis'))).toBe(false);

    const ini = readFileSync(path.join(pcsx2DataDir(), 'inis', 'PCSX2.ini'), 'utf-8');
    expect(ini).toContain('[UI]');
    expect(ini).toContain('SetupWizardIncomplete = false');
    expect(ini).toContain('SettingsVersion = 1');
    expect(ini).toContain('InhibitScreensaver = true');
    expect(ini).toContain('[AutoUpdater]');
    expect(ini).toContain('CheckAtStartup = false');
    expect(ini).toContain('[EmuCore]');
    expect(ini).toContain('EnableDiscordPresence = false');
    expect(ini).toContain('[EmuCore/GS]');
    expect(ini).toContain('pcrtc_antiblur = true');
    expect(ini).toContain('StartFullscreen = true');

    // No RA credentials are configured in this E2E run, which gates the
    // whole [Achievements] block off.
    expect(ini).not.toContain('[Achievements]');
    // [Folders] Bios is the profile's FIRST firmware directory ("bios" in
    // emulator-autoprofiles.json), resolved against the data root
    // (autoconfig/mod.rs `sync_new_emulator`, doc 05 step 15). Under layout
    // v2 that directory is a LINK into `saves/<Emulator>/`, and the writer
    // records where the link lands.
    expect(ini).toContain('[Folders]');
    expect(ini).toContain(`Bios = ${realpathSync(path.join(pcsx2DataDir(), 'bios'))}`);
    expect(realpathSync(path.join(pcsx2DataDir(), 'bios'))).toBe(
      realpathSync(path.join(dataDir(), 'library', 'saves', PCSX2_NAME, 'bios')),
    );
  });

  it('plays the seeded PS2 game with the installed PCSX2 as the system’s emulator', async () => {
    await waitForConfigLine(`"${PLATFORM}" = "${PCSX2_NAME}"`);
    await closeSystems();

    await $(testId('server-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never came back after leaving Settings',
    });
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-401')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-401')).click();
    await $(testId('details-play')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-play never appeared for the pre-seeded installed game',
    });

    await $(testId('details-play')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-playing-chip never appeared after Play',
    });

    await browser.waitUntil(() => existsSync(argvFile()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the installed PCSX2 stub never wrote its argv file',
    });
    const argv = readFileSync(argvFile(), 'utf-8').trim().split('\n');
    // The profile's args template, with %rom% substituted.
    expect(argv).toEqual(['-portable', '-fullscreen', '-batch', romPath()]);

    await $(testId('details-stop')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: REAP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'details-playing-chip never cleared after Stop within the reaper window',
    });
    await $(testId('details-close')).click();
  });

  it('installs Redream by scraping its download page and extracting the tar.gz', async () => {
    await openSystems();
    await pick('dc', `install:${REDREAM_SOURCE}`);
    await waitForCompleted(2);
    await browser.waitUntil(async () => (await group('dc', 'Installed')).includes(`use:${REDREAM_NAME}`), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Dreamcast picker never listed Redream under Installed',
    });

    expect((await sheetFields(REDREAM_NAME)).path).toBe(redreamPath());
    expect(existsSync(redreamPath())).toBe(true);
    // The recorded executable is the bare, executable-bit tar member: the
    // extraction kept the bit, which is what made it selectable at all.
    expect(statSync(redreamPath()).mode & 0o111).not.toBe(0);
    // The archive is deleted once its contents are merged in.
    expect(existsSync(path.join(emulatorsDir(), REDREAM_NAME, `${REDREAM_NAME}-nightly.gz`))).toBe(
      false,
    );
    await closeSystems();
  });

  it('checks PCSX2 for a newer release and updates it in place', async () => {
    await openSystems();
    await openRow('ps2');
    const line = await findLine(PCSX2_NAME);

    // From here on the mock forge serves a NEWER PCSX2 release than the one
    // installed above, with the same asset name and different bytes.
    const bump = await fetch(`${forgeUrl()}/__e2e__/pcsx2-release/updated`, { method: 'POST' });
    expect(bump.status).toBe(200);
    const logBeforeCheck = readForgeLog();

    // The check runs once a session on its own, and again on Check for
    // updates; its answer puts Update beside the emulator.
    await $(testId('systems-check-updates')).click();
    const updateBtn = $(testId(`sys-update-${line}`));
    await updateBtn.waitForDisplayed({
      timeout: TRANSITION_TIMEOUT * 2,
      timeoutMsg: 'the version check never offered an update',
    });
    await expect(updateBtn).toHaveText(expect.stringContaining('v9.9.1-e2e'));
    await expect($(testId(`sys-emu-version-${line}`))).toHaveText(expect.stringContaining('v9.9-e2e'));
    await expect($(testId('sys-status-ps2'))).toHaveAttribute('data-status', 'update');

    await updateBtn.click();
    // The completion toast is worded for an UPDATE, not a first install.
    await browser.waitUntil(
      async () =>
        (await $(testId('toast-region')).isExisting()) &&
        (await $(testId('toast-region')).getText()).includes(
          `Updated emulator '${PCSX2_NAME}' from source.`,
        ),
      {
        timeout: EMULATOR_INSTALL_TIMEOUT,
        timeoutMsg: 'the update never produced its completion toast',
      },
    );
    await waitForCompleted(3);
    // Current now: Update goes, and the version says so.
    await openRow('ps2');
    await $(testId(`sys-update-${line}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'Update stayed offered after the update finished',
    });
    // The update is one drawer row of its own, and nothing else: an update
    // reports `fresh == false`, which suppresses the firmware pass.
    await expect($(testId('download-row-4'))).not.toExist();

    // Same install directory, same file: the new AppImage replaced the old
    // one in place rather than landing in a per-release directory.
    await waitForConfigLine('source_installed_tag = "v9.9.1-e2e"');
    expect(readFileSync(pcsx2Path(), 'utf-8')).toContain('mock forge stub: pcsx2 (v9.9.1-e2e)');
    // Autoconfig ran again against the data root and still wrote no
    // `portable.ini` beside the AppImage.
    expect(existsSync(path.join(pcsx2DataDir(), 'inis', 'PCSX2.ini'))).toBe(true);
    expect(existsSync(path.join(pcsx2Dir(), 'portable.ini'))).toBe(false);

    const config = readFileSync(configPath(), 'utf-8');
    // The user-owned fields and the platform default are untouched.
    expect(config).toContain('-portable -fullscreen -batch');
    expect(config).toContain(`"${PLATFORM}" = "${PCSX2_NAME}"`);
    expect(config).toContain('source_release_tag = "latest"');

    // One release request for the check, one for the install's own resolve,
    // and one download of the new tag's asset.
    const added = readForgeLog().slice(logBeforeCheck.length);
    const occurrences = (haystack: string, needle: string) => haystack.split(needle).length - 1;
    expect(occurrences(added, '/api.github.com/repos/PCSX2/pcsx2/releases/latest')).toBe(2);
    expect(
      occurrences(added, `/github.com/PCSX2/pcsx2/releases/download/v9.9.1-e2e/${PCSX2_ASSET}`),
    ).toBe(1);
    expect(added).not.toContain('AUTH-HEADER-SEEN');
  });

  it('reached the forge with no credential, and installed only forge-served bytes', async () => {
    const log = readForgeLog();
    // The forge client carries no Authorization header, ever: mock-forge.mjs
    // answers 500 and logs this marker if one ever arrives.
    expect(log).not.toContain('AUTH-HEADER-SEEN');
    // Both providers went through the forge, not through RomM.
    expect(log).toContain('/api.github.com/repos/PCSX2/pcsx2/releases/latest');
    expect(log).toContain(`/github.com/PCSX2/pcsx2/releases/download/v9.9-e2e/${PCSX2_ASSET}`);
    expect(log).toContain('/redream.io/download');
    expect(log).toContain('/redream.io/download/redream.x86_64-linux-v1.5.0-1000-gabcdef0.tar.gz');

    // Provenance, from the other end: both installed files are the mock
    // forge's own stub bytes. The mock RomM server serves nothing like them,
    // so these installs provably did not come from the RomM content
    // endpoints. (An in-spec assertion over the RomM request log itself is
    // not possible: mock-romm/server.mjs writes that log from close(), i.e.
    // after the spec process is gone — see task-8-report.md.)
    expect(readFileSync(pcsx2Path(), 'utf-8')).toContain('mock forge stub: pcsx2');
    expect(readFileSync(redreamPath(), 'utf-8')).toContain('mock forge stub: redream');
  });

  it('offers PCSX2 as a download again once the installed PCSX2 is removed', async () => {
    // Last case in the group (not right after the install), since the cases
    // above still need PCSX2 configured: the system's emulator and the launch
    // it drives. The previous case leaves the shell on Downloads.
    await openSystems();
    await removeEmulator(await findLine(PCSX2_NAME), PCSX2_NAME);

    // Removing an emulator removes its installed files, not just the config
    // entry. The removal runs BEFORE the config write; poll anyway, since
    // the line can disappear from a re-render the write triggers.
    await browser.waitUntil(() => !existsSync(pcsx2Dir()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the PCSX2 install directory survived the removal: ${pcsx2Dir()}`,
    });
    // A different install must not be touched.
    expect(existsSync(redreamPath())).toBe(true);

    // The picker must follow the config, not wait for another install job
    // to reach a terminal status.
    await browser
      .waitUntil(async () => (await group('ps2', 'Download')).includes(`install:${PCSX2_SOURCE}`), {
        timeout: TRANSITION_TIMEOUT,
      })
      .catch(async () => {
        throw new Error(
          `the PS2 picker never offered PCSX2 as a download again after removing it: ${JSON.stringify(await groups('ps2'))}`,
        );
      });
    expect(await group('ps2', 'Installed')).not.toContain(`use:${PCSX2_NAME}`);
  });

  it("installs GRID's own umu-launcher from the Windows row", async () => {
    // The previous case leaves Settings › Systems in front.
    await openSystems();
    await openRow('windows');
    const status = $(testId('compat-umu-status-systems'));
    await status.waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Windows row never showed its umu-run line',
    });
    const before = await status.getText();
    if (before.includes('system')) {
      // A host with its own umu-run reports it, and a launch would never
      // reach GRID's copy, so there is nothing to install here.
      console.log(`emulator-catalog: skipping the umu-launcher install, the host has one (${before})`);
      return;
    }
    expect(before).toBe('Proton launcher · umu-run: not installed');

    const install = $(testId('sys-umu-install'));
    expect(await install.getText()).toBe('Install');
    await install.click();

    await browser.waitUntil(async () => (await status.getText()) === 'Proton launcher · umu-run: GRID-managed 1.4.4-e2e', {
      timeout: EMULATOR_INSTALL_TIMEOUT,
      timeoutMsg: 'the umu-run line never reported the GRID-managed copy',
    });
    await browser.waitUntil(async () => (await install.getText()) === 'Reinstall', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the umu-run button never became Reinstall',
    });

    // The managed copy: under the compat-tools root, executable, recorded
    // in config with the resolved tag, and the forge's own stub bytes.
    const umuRun = path.join(dataDir(), 'compat-tools', 'umu-launcher', 'umu', 'umu-run');
    expect(existsSync(umuRun)).toBe(true);
    expect(statSync(umuRun).mode & 0o111).toBe(0o111);
    expect(readFileSync(umuRun, 'utf-8')).toContain('mock forge stub: umu-run');
    await waitForConfigLine('[umu_launcher]');
    expect(readFileSync(configPath(), 'utf-8')).toContain('release_tag = "1.4.4-e2e"');

    const log = readForgeLog();
    expect(log).toContain('/api.github.com/repos/Open-Wine-Components/umu-launcher/releases/latest');
    expect(log).toContain(
      '/github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.4-e2e/umu-launcher-1.4.4-e2e-zipapp.tar',
    );
    expect(log).not.toContain('AUTH-HEADER-SEEN');
  });
});
