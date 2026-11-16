import { existsSync, lstatSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';
import { connectRomm } from '../helpers/romm.js';
import { editEmulator, findLine, groups, lineKey, openRow, openSystems, pick, removeEmulator } from '../helpers/systems.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `server-emulator` (plan B7 / Q3; docs/server-emulator-packages.md): a
 * build on the server's "Emulators" platform is an emulator. RomM's
 * Emulators shelf is never a Library row (romm-storefront-plan 0.3), so the
 * build is installed from Settings › Systems, as a "from your server"
 * download on PS2's picker. It downloads, extracts under
 * `emulators/<Profile>/`, registers as an emulator entry, is configured, and
 * shows as PS2's emulator. Removing the entry uninstalls the build.
 *
 * The fixture set (`e2e/fixtures-server-emulator`) carries platform 1
 * "Emulators" with rom 801, the `ps2` folder, whose one build
 * `PCSX2 (v2.0.2) (arch-linux-x64).zip` holds the mock forge's PCSX2
 * AppImage stub (mock-romm/server.mjs). It is named to the packaging rules,
 * so the entry takes the PCSX2 profile's name and args. Platform 2 "Sony
 * PlayStation 2" is the system it serves.
 */
describe('server-emulator', () => {
  const PCSX2_NAME = 'PCSX2 (Playstation 2)';
  const APPIMAGE = 'pcsx2-v9.9-e2e-linux-appimage-x64-Qt.AppImage';

  const library = () => path.join(dataDir(), 'library');
  const packageDir = () => path.join(library(), 'emulators', PCSX2_NAME);
  /** The AppImage, wherever the build's folders put it under `packageDir`. */
  const exePath = (): string => {
    const walk = (dir: string): string | null => {
      for (const name of readdirSync(dir)) {
        const full = path.join(dir, name);
        if (name === APPIMAGE) return full;
        if (lstatSync(full).isDirectory()) {
          const hit = walk(full);
          if (hit) return hit;
        }
      }
      return null;
    };
    return (existsSync(packageDir()) && walk(packageDir())) || path.join(packageDir(), APPIMAGE);
  };
  /** PCSX2's AppImage roots its data at `<AppImage dir>/PCSX2`; `inis` is a
   *  user-data link into `saves/<Profile>/`, and the writer writes through it. */
  const savedIni = () => path.join(library(), 'saves', PCSX2_NAME, 'inis', 'PCSX2.ini');

  async function show(view: 'downloads') {
    await $(testId(`nav-${view}`)).click();
    await $(testId(`${view}-view`)).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the ${view} view never opened`,
    });
  }

  function configText(): string {
    try {
      return readFileSync(configPath(), 'utf-8');
    } catch {
      return '';
    }
  }

  before(async () => {
    await connectRomm({ libraryPath: library() });
  });

  it('installs the Emulators-platform package with the Emulator badge on its Downloads row', async () => {
    // RomM's Emulators shelf is never a Library rail row (romm-storefront-plan
    // 0.3): its builds are offered in Settings › Systems, as "from your
    // server" downloads on the systems they serve.
    await openSystems();
    await openRow('ps2');
    const download = (await groups('ps2')).Download ?? [];
    const server = download.find((o) => o.endsWith('· from your server'));
    expect(server).toBeDefined();
    await pick('ps2', server!.slice(0, server!.indexOf('=')));

    await show('downloads');
    await $(testId('download-row-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no downloads row appeared for the package install',
    });
    await expect($(testId('download-kind-1'))).toHaveText('Emulator');
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the package install never reached Completed' },
    );
  });

  it('extracts the build under emulators/<Profile> and makes the AppImage executable', () => {
    expect(existsSync(exePath())).toBe(true);
    expect(statSync(exePath()).mode & 0o111).not.toBe(0);
  });

  it('registers the package as the matched profile, shown in Settings › Systems', async () => {
    await openSystems();
    // PS2's emulator: the PS2 row shows it.
    expect(await findLine(PCSX2_NAME)).toBe(lineKey('Sony PlayStation 2'));
    await editEmulator(await findLine(PCSX2_NAME));
    await expect($(testId('emu-form-name'))).toHaveValue(PCSX2_NAME);
    await expect($(testId('emu-form-path'))).toHaveValue(exePath());
    await expect($(testId('emu-form-args'))).toHaveValue('-portable -fullscreen -batch "%rom%"');
    await $(testId('emu-form-cancel')).click();
    await $(testId('systems-sheet')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('configures it like a catalog install: platform default and PCSX2.ini through the saves link', async () => {
    await browser.waitUntil(() => configText().includes(`"Sony PlayStation 2" = "${PCSX2_NAME}"`), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the defaults backfill never made the package the PS2 default',
    });
    // Its update source is the server's folder, not the catalog's forge.
    expect(configText()).toMatch(/source_id = "romm:ps2\//);
    expect(configText()).toContain('source_installed_tag = "2.0.2"');

    await browser.waitUntil(() => existsSync(savedIni()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'autoconfig never wrote PCSX2.ini into saves/ through the inis link',
    });
    const ini = readFileSync(savedIni(), 'utf-8');
    expect(ini).toContain('[UI]');
    expect(ini).toContain('SetupWizardIncomplete = false');
  });

  it('removing the emulator uninstalls the package and keeps its saves', async () => {
    await removeEmulator(await findLine(PCSX2_NAME), PCSX2_NAME);

    expect(existsSync(packageDir())).toBe(false);
    expect(configText()).not.toContain(PCSX2_NAME);
    // The user data lived behind the links, under saves/, and survives.
    expect(existsSync(savedIni())).toBe(true);
  });
});
