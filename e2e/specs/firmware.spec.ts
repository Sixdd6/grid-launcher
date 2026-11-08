import { existsSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIRMWARE_TIMEOUT,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';
import { addManual, findLine, lineKey, openSystems } from '../helpers/systems.js';
import { waitForToast } from '../helpers/toasts.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `firmware`: the two background firmware triggers
 * (app/src-tauri/src/firmware_service.rs), neither of which the user asks
 * for directly.
 *
 * 1. **Per game.** Installing rom 801 (PlayStation) finalizes, and the
 *    `game_finalized_hook` runs a firmware pass for that platform against
 *    the seeded DuckStation default. Its profile routes firmware to `bios`
 *    beside the executable, so the fixture's `scph5501.bin` appears at
 *    `<stubs>/duckstation/bios/`. Nothing in the UI announces this, so the
 *    assertion polls the filesystem.
 *
 * 2. **Adding RPCS3 by hand.** `commands::save_emulator` fires
 *    `spawn_ps3_firmware` for a newly ADDED RPCS3 entry, which — unlike
 *    trigger 1 — is user-visible: it admits its own `PS3 Firmware` drawer
 *    row (`admit_external`) because the PUP is large. Once that row
 *    completes, Settings › Systems re-queries `rpcs3_firmware_status` and
 *    reveals the note and its Install button, which hands the PUP to
 *    RPCS3's own `--installfw`.
 *
 * The seed writes the RPCS3 stub file but deliberately NOT a config entry
 * for it: adding that entry with Add an emulator manually is the trigger.
 */
describe('firmware', () => {
  const stubs = () => path.join(dataDir(), 'stubs');
  const rpcs3Exe = () => path.join(stubs(), 'rpcs3', 'rpcs3');

  // The views no longer stack (design §3): one pill click swaps which
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
  });

  it("downloads the platform's BIOS beside the default emulator after a game install", async () => {
    await showServer();
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-801')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-801')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 801',
    });
    await $(testId('details-install')).click();

    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the PS1 install never reached Completed' },
    );

    // The per-game firmware pass runs in the background off the finalize
    // hook and reports nothing to the UI, so poll for its one side effect.
    const bios = path.join(stubs(), 'duckstation', 'bios', 'scph5501.bin');
    await browser.waitUntil(() => existsSync(bios), {
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: `the per-game firmware pass never wrote ${bios}`,
    });

    await showServer();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it("installs a platform's firmware from its system's row in Settings › Systems", async () => {
    // Firmware installs beside the system's emulator (user ruling
    // 2026-10-30). The Server header's chip links there instead of
    // installing itself.
    const bios = path.join(stubs(), 'duckstation', 'bios', 'scph5501.bin');
    rmSync(bios, { force: true });
    // The first case's pass put the BIOS in place, so the row starts out
    // Installed; with it gone again, the row offers Install.

    await showServer();
    await $(testId('platform-btn-1')).click();
    await $(testId('server-firmware-chip')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await $(testId('server-firmware-chip')).getText()).startsWith('Firmware: '), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the firmware chip never reported the server firmware',
    });
    await $(testId('server-firmware-chip')).click();
    // The chip opens Settings › Systems on PlayStation's row, opened and in view.
    await $(testId('settings-page-systems')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the firmware chip did not open Settings › Systems',
    });
    await browser.waitUntil(
      async () => ((await $(testId('sys-row-ps')).getAttribute('class')) ?? '').split(/\s+/).includes('open'),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the firmware chip did not open PlayStation's row" },
    );
    await expect($(testId('sys-row-ps'))).toBeDisplayedInViewport();
    await expect($(testId(`sys-emu-name-${lineKey('PlayStation')}`))).toHaveText('DuckStation (Playstation 1)');

    const install = $(testId('sys-firmware-install-ps'));
    await install.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "PlayStation's row never listed its firmware",
    });
    await install.click();
    // The click says at once that the install started …
    await waitForToast(/Installing .* firmware for DuckStation/, {
      timeoutMsg: 'Install firmware never said it had started',
    });
    await browser.waitUntil(() => existsSync(bios), {
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: `Install firmware never wrote ${bios}`,
    });
    // The pass-finished event re-reads the status, which now finds the
    // BIOS in place: the row says Installed instead of offering Install again.
    await $(testId('sys-firmware-installed-ps')).waitForExist({
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: 'the row never said the firmware was installed',
    });
    await expect(install).not.toExist();
    // … and when it has finished.
    await waitForToast(/firmware installed for DuckStation/, {
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: 'Install firmware never said it had finished',
    });
  });

  it('fetches the PS3 PUP through its own drawer row when RPCS3 is added by hand', async () => {
    await openSystems();
    // Something else, with the name typed: the entry has to stay named
    // `RPCS3`, which is how the page finds it below.
    await addManual({ name: 'RPCS3', path: rpcs3Exe() });
    await findLine('RPCS3');

    await showDownloads();

    // `spawn_ps3_firmware` admits its own external download row, titled
    // verbatim "PS3 Firmware" (firmware_service.rs's PS3_FIRMWARE_TITLE).
    await $(testId('download-row-2')).waitForExist({
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: 'adding RPCS3 never admitted a PS3 firmware download row',
    });
    expect(await $(`${testId('download-row-2')} .title`).getText()).toBe('PS3 Firmware');
    expect(await $(testId('download-kind-2')).getText()).toBe('Firmware');

    await browser.waitUntil(
      async () => (await $(testId('download-detail-2')).getText()).startsWith('Completed'),
      {
        timeout: FIRMWARE_TIMEOUT,
        timeoutMsg: 'the PS3 firmware row never reached Completed',
      },
    );
    expect(existsSync(path.join(stubs(), 'rpcs3', 'PS3UPDAT.PUP'))).toBe(true);
  });

  it("hands the downloaded PUP to RPCS3's own --installfw", async () => {
    await openSystems();
    await findLine('RPCS3');
    // The note and its Install button only render once
    // `rpcs3_firmware_status` reports a PUP; the page re-queries it when a
    // `firmware`-kind download entry completes.
    let button = '';
    await browser.waitUntil(
      async () => {
        button = await browser.execute(() => {
          const hit = Array.from(document.querySelectorAll('[data-testid^="sys-emu-name-"]')).find(
            (el) => el.textContent?.trim() === 'RPCS3',
          );
          const el = hit?.closest('li')?.querySelector('[data-testid^="sys-ps3-firmware-install-"]');
          return (el as HTMLElement | null)?.dataset.testid ?? '';
        });
        return button !== '';
      },
      { timeout: FIRMWARE_TIMEOUT, timeoutMsg: 'RPCS3 never showed the downloaded-firmware note' },
    );
    const note = button.replace('sys-ps3-firmware-install-', 'sys-ps3-firmware-');
    await expect($(testId(note))).toHaveText(expect.stringContaining('PS3 firmware downloaded — click Install to activate it.'));
    await $(testId(button)).click();

    await waitForToast('PS3 firmware installation started — follow the RPCS3 dialog to complete.', {
      timeoutMsg: 'clicking Install never said the PS3 firmware installation started',
    });

    const argvLog = path.join(dataDir(), 'rpcs3-argv.log');
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(argvLog, 'utf-8').includes('--installfw');
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the RPCS3 stub was never spawned with --installfw',
      },
    );
    const argv = readFileSync(argvLog, 'utf-8').split('\n').filter(Boolean);
    expect(argv[0]).toBe('--installfw');
    expect(argv[1].endsWith('PS3UPDAT.PUP')).toBe(true);
  });
});
