import { existsSync, readFileSync, statSync } from 'node:fs';
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
import { editEmulator, findLine, lineKey, openSystems, removeEmulator } from '../helpers/systems.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `server-emulator` (plan B7 / Q3): a ROM on the server's "Emulators"
 * platform is an emulator build. Installing it from the Server view
 * downloads and extracts it like a game (under `games/Emulators/<stem>`),
 * then registers it as an emulator entry, configures it, and shows it as
 * PS2's emulator in Settings › Systems. Removing the entry uninstalls the
 * package.
 *
 * The fixture set (`e2e/fixtures-server-emulator`) carries platform 1
 * "Emulators" with rom 801, whose `pcsx2-server.zip` holds the mock forge's
 * PCSX2 AppImage stub (mock-romm/server.mjs). Its file name matches the
 * PCSX2 catalog profile's `match_tokens`, so the entry takes the profile's
 * name and args. Platform 2 "Sony PlayStation 2" is there so the defaults
 * backfill has a platform the profile matches.
 */
describe('server-emulator', () => {
  const PCSX2_NAME = 'PCSX2 (Playstation 2)';
  const APPIMAGE = 'pcsx2-v9.9-e2e-linux-appimage-x64-Qt.AppImage';

  const library = () => path.join(dataDir(), 'library');
  const packageDir = () => path.join(library(), 'games', 'Emulators', 'pcsx2-server');
  const exePath = () => path.join(packageDir(), 'PCSX2', APPIMAGE);
  /** PCSX2's AppImage roots its data at `<AppImage dir>/PCSX2`; `inis` is a
   *  user-data link into `saves/<Profile>/`, and the writer writes through it. */
  const savedIni = () => path.join(library(), 'saves', PCSX2_NAME, 'inis', 'PCSX2.ini');

  async function show(view: 'server' | 'downloads') {
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
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    // A fresh profile has no library folder: Connect asks for one.
    await $(testId('connect-library-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('connect-library-path')).setValue(library());
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never rendered a platform button after connecting',
    });
  });

  it('installs the Emulators-platform package with the Emulator badge on its Downloads row', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-801')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-801')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 801',
    });
    await $(testId('details-install')).click();

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

  it('extracts the package under games/Emulators and makes the AppImage executable', () => {
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
    // No forge update source: a server package is not a catalog install.
    expect(configText()).not.toContain('source_id');

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
