import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, INSTALL_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openLibrary, startScreen } from '../helpers/romm.js';
import { dismissToasts, waitForToast } from '../helpers/toasts.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const within = (row: string, id: string) => `${testId(row)} ${testId(id)}`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'romm-emulators');

/** A screenshot for the design parity check (ctr-out/ is git-ignored). */
async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/**
 * Stage `server-emulator`, part B (RommStorefront README "RomM › Emulators";
 * docs/server-emulator-packages.md): the server's Emulators platform is a
 * row in Library › RomM, sorted by name among the platforms, and its view
 * installs the builds. Part A left RomM connected and PCSX2 removed.
 *
 * The fixture set (`e2e/fixtures-server-emulator`) has on platform 1
 * "Emulators": `ps2` (PCSX2 v2.0.2 for Linux x64, an emulator; the same
 * version for Windows x64, an other build), `pico` (Pico-8 for 32-bit ARM
 * only, an other build) and `multi` (Ares Nightly, named to the rule but
 * an emulator GRID doesn't know, and xemu, not named for GRID: two Other
 * files). So the row counts 3 (1 emulator + 2 other files).
 */
describe('server-emulator (b): Library › RomM › Emulators', () => {
  const library = () => path.join(dataDir(), 'library');
  const emulatorsFolder = () => path.join(library(), 'games', 'Emulators');
  const xemu = () => path.join(emulatorsFolder(), 'multi', 'xemu-v0.8.5-x86_64.AppImage');
  const config = () => {
    try {
      return readFileSync(configPath(), 'utf-8');
    } catch {
      return '';
    }
  };

  async function railOrder(): Promise<string[]> {
    return browser.execute(() =>
      [...document.querySelectorAll('[data-testid="library-rail"] button')].map((b) => b.getAttribute('data-testid') ?? ''),
    );
  }

  async function rowState(row: string): Promise<string | null> {
    return $(testId(row)).getAttribute('data-state');
  }

  async function waitState(row: string, state: string, timeout = INSTALL_TIMEOUT): Promise<void> {
    let last: string | null = null;
    await browser
      .waitUntil(
        async () => {
          last = await rowState(row);
          return last === state;
        },
        { timeout, interval: 150 },
      )
      .catch(() => {
        throw new Error(`${row} never reached ${state} (last: ${last})`);
      });
  }

  before(async () => {
    expect(await startScreen()).toBe('shell');
    await openLibrary();
  });

  it('shows an Emulators row sorted by name among the RomM platforms, counting what this computer can install', async () => {
    await $(testId('library-rail-platform-emulators')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Emulators row never appeared in the RomM group',
    });
    await browser.waitUntil(async () => (await $(testId('library-rail-count-platform-emulators')).isExisting()), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Emulators row never showed its count',
    });
    await expect($(testId('library-rail-count-platform-emulators'))).toHaveText('3');
    const order = (await railOrder()).filter((id) => id.startsWith('library-rail-romm-') || id.startsWith('library-rail-platform-'));
    expect(order).toEqual([
      'library-rail-romm-installed',
      'library-rail-romm-discover',
      'library-rail-platform-emulators',
      'library-rail-platform-ps2',
    ]);
    await expect($(testId('library-rail-platform-emulators'))).toHaveText(expect.stringContaining('Emulators'));
  });

  it('opens a list, not covers: header, counts, the Builds chip and no platform-only controls', async () => {
    await $(testId('library-rail-platform-emulators')).click();
    await $(testId('romm-emulators-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await $(testId('romm-emulators-counts')).getText()) === '1 emulator · 2 other files · 0 installed', {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Emulators counts line never read "1 emulator · 2 other files · 0 installed"',
    });
    await expect($(testId('romm-emulators-builds-chip'))).toHaveText('Builds for Linux x64');
    await expect($(testId('romm-emulators-systems-chip'))).toHaveText('Settings › Systems');
    await expect($(testId('romm-emulators-search'))).toHaveAttribute('placeholder', 'Search emulators or files');
    // Not here: firmware and emulator chips, Backup all, Select, Size (the platform screen's ids).
    for (const id of ['server-firmware-chip', 'server-emulator-chip', 'server-backup-platform', 'server-select', 'server-size']) {
      await expect($(testId(id))).not.toExist();
    }
  });

  it('lists the emulator with its system, version and file, and the Other files with why', async () => {
    const pcsx2 = testId('romm-emu-pcsx2');
    // A single-system folder names the server's platform for its slug.
    await expect($(pcsx2)).toHaveText(expect.stringContaining('Sony PlayStation 2'));
    await expect($(pcsx2)).toHaveText(expect.stringContaining('v2.0.2'));
    await expect($(pcsx2)).toHaveText(expect.stringContaining('PCSX2 (v2.0.2) (arch-linux-x64).zip'));
    await expect($(within('romm-emu-pcsx2', 'romm-emu-install'))).toHaveText('Install');

    await expect($(testId('romm-emu-file-8041'))).toHaveText(expect.stringContaining('multi folder'));
    await expect($(testId('romm-emu-file-8041'))).toHaveText(expect.stringContaining('GRID doesn’t know an emulator called “Ares Nightly”'));
    await expect($(testId('romm-emu-file-8042'))).toHaveText(expect.stringContaining('Not named for GRID'));
    await expect($(within('romm-emulators-other-files', 'romm-emulators-guide'))).toHaveText('How to name builds');
    await shot('desktop-list');
  });

  it('keeps the builds for other computers in a closed disclosure, with reasons', async () => {
    const builds = $(testId('romm-emulators-builds'));
    await expect(builds).toHaveText(expect.stringContaining('2 other builds GRID doesn’t install on this computer'));
    expect(await builds.getAttribute('open')).toBeNull();
    await $(`${testId('romm-emulators-builds')} summary`).click();
    await expect(builds).toHaveText(expect.stringContaining('PCSX2 (v2.0.2) (arch-windows-x64).zip'));
    await expect(builds).toHaveText(expect.stringContaining('For Windows x64'));
    await expect(builds).toHaveText(expect.stringContaining('Pico-8 (v0.2.7a6) (arch-linux-armhf).zip'));
    await expect(builds).toHaveText(expect.stringContaining('For Linux 32-bit ARM'));
    await shot('desktop-builds-open');
  });

  it('searches emulators and files; the other builds hide while searching', async () => {
    await $(testId('romm-emulators-search')).setValue('ares');
    await expect($(testId('romm-emu-file-8041'))).toExist();
    await expect($(testId('romm-emu-pcsx2'))).not.toExist();
    await expect($(testId('romm-emulators-builds'))).not.toExist();
    await $(testId('romm-emulators-search')).setValue('zzz');
    await expect($(testId('romm-emulators-no-match'))).toHaveText('No emulators or files match “zzz”');
    await $(testId('romm-emulators-search')).clearValue();
    await $(testId('romm-emulators-search')).setValue(' ');
    await $(testId('romm-emu-pcsx2')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('installs the emulator through the server-emulator path and registers it', async () => {
    await dismissToasts();
    await $(within('romm-emu-pcsx2', 'romm-emu-install')).click();
    await waitState('romm-emu-pcsx2', 'installed');
    await waitForToast('PCSX2 v2.0.2 installed from RomM · set up for Sony PlayStation 2', { timeout: INSTALL_TIMEOUT });
    expect(config()).toMatch(/source_id = "romm:ps2\//);
    await expect($(testId('romm-emu-pcsx2'))).toHaveText(expect.stringContaining('Set up in Settings › Systems'));
    await expect($(within('romm-emu-pcsx2', 'romm-emu-uninstall'))).toHaveText('Uninstall…');
    await browser.waitUntil(async () => (await $(testId('romm-emulators-counts')).getText()) === '1 emulator · 2 other files · 1 installed', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the counts line never counted the installed emulator',
    });
  });

  it('installs an Other file as a plain file in the library’s Emulators folder, unregistered', async () => {
    await dismissToasts();
    await $(within('romm-emu-file-8042', 'romm-emu-install')).click();
    await waitState('romm-emu-file-8042', 'installed');
    await waitForToast('xemu-v0.8.5-x86_64.AppImage installed in your library’s Emulators folder', { timeout: INSTALL_TIMEOUT });
    expect(existsSync(xemu())).toBe(true);
    expect(config()).not.toContain('xemu-v0.8.5');
    await expect($(within('romm-emu-file-8042', 'romm-emu-show-folder'))).toHaveText('Show folder');
    await shot('desktop-installed');
  });

  it('emulator files never join RomM › Installed or All games', async () => {
    await expect($(testId('library-rail-count-romm-installed'))).toHaveText('0');
    await expect($(testId('library-rail-count-all'))).toHaveText('0');
  });

  it('uninstalls with an inline confirm: the Other file, then the emulator', async () => {
    await $(within('romm-emu-file-8042', 'romm-emu-uninstall')).click();
    await expect($(within('romm-emu-file-8042', 'romm-emu-confirm'))).toHaveText(
      expect.stringContaining('Uninstall xemu-v0.8.5-x86_64.AppImage?'),
    );
    await shot('desktop-confirm');
    await $(within('romm-emu-file-8042', 'romm-emu-confirm-uninstall')).click();
    await waitState('romm-emu-file-8042', 'none', TRANSITION_TIMEOUT);
    await waitForToast('xemu-v0.8.5-x86_64.AppImage uninstalled');
    expect(existsSync(xemu())).toBe(false);

    await $(within('romm-emu-pcsx2', 'romm-emu-uninstall')).click();
    await expect($(within('romm-emu-pcsx2', 'romm-emu-confirm'))).toHaveText(expect.stringContaining('Uninstall PCSX2 v2.0.2?'));
    await expect($(within('romm-emu-pcsx2', 'romm-emu-confirm'))).toHaveText(
      expect.stringContaining('Sony PlayStation 2 games set to it can’t start until another emulator is picked.'),
    );
    await $(within('romm-emu-pcsx2', 'romm-emu-confirm-uninstall')).click();
    await waitState('romm-emu-pcsx2', 'none', TRANSITION_TIMEOUT);
    await waitForToast('PCSX2 uninstalled');
    expect(config()).not.toMatch(/source_id = "romm:ps2\//);
  });

  it('Settings › Library has the Config folder block with its path and Open config folder', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('settings-nav-library')).click();
    await $(testId('settings-library-pane')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(`${testId('settings-config-folder')} h3`)).toHaveText('Config folder');
    await expect($(testId('config-folder-path'))).toHaveText(path.dirname(configPath()));
    await expect($(testId('config-folder-open'))).toHaveText('Open config folder');
    await expect($(testId('config-folder-open'))).toBeEnabled();
    await shot('desktop-config-folder');
  });
});
