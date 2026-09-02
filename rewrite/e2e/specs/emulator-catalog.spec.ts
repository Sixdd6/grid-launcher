import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  REAP_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

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
 * autoprofile catalog, end to end, against `mock-romm/mock-forge.mjs`.
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
 *   in `<library>/Emulators/PCSX2 (Playstation 2)-latest/`.
 * - `Redream (Sega Dreamcast)` — a `direct` source, resolved by scraping the
 *   mock's HTML download page with the catalog's own regex, then extracted
 *   from a real tar.gz.
 *
 * `rewrite/e2e/seed/emulator-catalog-seed.mjs` pre-seeds the library path
 * and one installed game (rom 401, "Gran Turismo 3" on "Sony PlayStation 2",
 * from e2e/fixtures-emulator-catalog) so the freshly installed PCSX2 can be
 * made that platform's default and actually launched.
 */
describe('emulator-catalog', () => {
  const PLATFORM = 'Sony PlayStation 2';
  const PCSX2_NAME = 'PCSX2 (Playstation 2)';
  const PCSX2_ROW = 'emulator-row-pcsx2-(playstation-2)';
  const PCSX2_ASSET = 'pcsx2-v9.9-e2e-linux-appimage-x64-Qt.AppImage';
  const REDREAM_NAME = 'Redream (Sega Dreamcast)';
  const REDREAM_ROW = 'emulator-row-redream-(sega-dreamcast)';

  const romPath = () => path.join(dataDir(), 'library', PLATFORM, 'Gran Turismo 3', 'game.iso');
  const emulatorsDir = () => path.join(dataDir(), 'library', 'Emulators');
  const pcsx2Path = () =>
    path.join(emulatorsDir(), `${PCSX2_NAME}-latest`, PCSX2_ASSET);
  /**
   * The tar.gz member is the bare `redream` the real tarball ships. Picking
   * it exercises `launchable_installed_file` (grid-core
   * launch/emu_install.rs): on unix an extracted file with no `.` in its
   * name and its executable bit set is launchable, alongside the reference's
   * .exe/.bat/.cmd/.ps1/.sh/.AppImage suffix set.
   */
  const redreamPath = () => path.join(emulatorsDir(), `${REDREAM_NAME}-nightly`, 'redream');

  const argvFile = (): string => {
    const value = process.env.GRID_E2E_ARGV_FILE;
    if (!value) {
      throw new Error('GRID_E2E_ARGV_FILE is not set — run this through rewrite/scripts/e2e.sh');
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

  async function openEmulators() {
    await $(testId('emulators-open')).click();
    await $(testId('emulators-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators panel never opened',
    });
  }

  async function closeEmulators() {
    await $(testId('emulators-close')).click();
    await $(testId('emulators-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the emulators panel never closed',
    });
  }

  /** Opens the Add form, which starts on the catalog (Install) tab. */
  async function openCatalog() {
    await $(testId('emulator-add')).click();
    await $(testId('emu-catalog-search')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the catalog Install tab never rendered its search box',
    });
  }

  /** See launch.spec.ts: a protocol click on an <option> fires no `change`. */
  async function selectValue(testIdName: string, value: string) {
    await browser.execute(
      (selector, val) => {
        const el = document.querySelector(selector) as HTMLSelectElement | null;
        if (!el) throw new Error(`no element matched ${selector}`);
        el.value = val;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      },
      testId(testIdName),
      value,
    );
  }

  async function setSearch(value: string) {
    await $(testId('emu-catalog-search')).setValue(value);
  }

  /** Waits for one downloads-drawer row to reach `Completed`. */
  async function waitForCompleted(entryId: number) {
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

    // Opened once, up front, and left open: every install below is asserted
    // through it, and the emulators panel renders over it without hiding it
    // from the DOM.
    await $(testId('downloads-footer')).click();
    await $(testId('downloads-drawer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads drawer never opened',
    });
  });

  it('lists catalog entries on the Install tab and narrows them by search', async () => {
    await openEmulators();
    await openCatalog();

    await expect($(testId('emu-add-tab-install'))).toHaveAttribute('aria-selected', 'true');
    await expect($(testId('emu-add-tab-manual'))).toExist();
    await expect($(testId('emu-catalog-install-PCSX2-pcsx2'))).toExist();
    await expect($(testId('emu-catalog-install-inolen-redream'))).toExist();

    await setSearch('pcsx2');
    await $(testId('emu-catalog-install-inolen-redream')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'searching for "pcsx2" left the Redream row visible',
    });
    await expect($(testId('emu-catalog-install-PCSX2-pcsx2'))).toExist();

    await setSearch('');
    await $(testId('emu-catalog-install-inolen-redream')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clearing the search never restored the full catalog',
    });
  });

  it('installs PCSX2 from the catalog and marks it installed', async () => {
    await $(testId('emu-catalog-install-PCSX2-pcsx2')).click();
    await waitForCompleted(1);

    // The catalog re-reads itself when an emulator job reaches a terminal
    // status, so the row flips to a disabled "Installed" button in place.
    await $(testId('emu-catalog-installed-PCSX2-pcsx2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the PCSX2 catalog row never flipped to Installed',
    });
    await expect($(testId('emu-catalog-installed-PCSX2-pcsx2'))).toBeDisabled();
    await expect($(testId('emu-catalog-install-PCSX2-pcsx2'))).not.toExist();

    // The AppImage is kept as-is (never extracted), under an install
    // directory named from the CONFIGURED tag ("latest"), and the config
    // entry carries the profile's args verbatim.
    expect(existsSync(pcsx2Path())).toBe(true);
    await waitForConfigLine(pcsx2Path());

    // Reopened because the panel loads its emulator list once, on mount.
    await closeEmulators();
    await openEmulators();
    await $(testId(PCSX2_ROW)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the installed PCSX2 entry never appeared in the emulator list',
    });
    const rowText = await $(testId(PCSX2_ROW)).getText();
    expect(rowText).toContain(PCSX2_NAME);
    expect(rowText).toContain(pcsx2Path());
    expect(rowText).toContain('-portable -fullscreen -batch "%rom%"');
  });

  it('plays the seeded PS2 game with the installed PCSX2 as the platform default', async () => {
    await selectValue('default-select-1', PCSX2_NAME);
    await waitForConfigLine(`"${PLATFORM}" = "${PCSX2_NAME}"`);
    await closeEmulators();

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
    await openEmulators();
    await openCatalog();
    await setSearch('redream');
    await $(testId('emu-catalog-install-inolen-redream')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Redream catalog row never rendered',
    });

    await $(testId('emu-catalog-install-inolen-redream')).click();
    await waitForCompleted(2);
    await $(testId('emu-catalog-installed-inolen-redream')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Redream catalog row never flipped to Installed',
    });

    await closeEmulators();
    await openEmulators();
    await $(testId(REDREAM_ROW)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the installed Redream entry never appeared in the emulator list',
    });
    expect(await $(testId(REDREAM_ROW)).getText()).toContain(redreamPath());
    expect(existsSync(redreamPath())).toBe(true);
    // The recorded executable is the bare, executable-bit tar member: the
    // extraction kept the bit, which is what made it selectable at all.
    expect(statSync(redreamPath()).mode & 0o111).not.toBe(0);
    // The archive is deleted once its contents are merged in.
    expect(existsSync(path.join(emulatorsDir(), `${REDREAM_NAME}-nightly`, `${REDREAM_NAME}-nightly.gz`))).toBe(false);
    await closeEmulators();
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
});
