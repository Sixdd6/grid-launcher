import { existsSync, readFileSync } from 'node:fs';
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
 *    completes, the Emulators panel re-queries `rpcs3_firmware_status` and
 *    reveals the note and the `Install PS3 Firmware` button, which hands the
 *    PUP to RPCS3's own `--installfw`.
 *
 * The seed writes the RPCS3 stub file but deliberately NOT a config entry
 * for it: adding that entry through the Emulators form is the trigger.
 */
describe('firmware', () => {
  const stubs = () => path.join(dataDir(), 'stubs');
  const rpcs3Exe = () => path.join(stubs(), 'rpcs3', 'rpcs3');

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
  });

  it("downloads the platform's BIOS beside the default emulator after a game install", async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-801')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-801')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 801',
    });
    await $(testId('details-install')).click();

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

    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('fetches the PS3 PUP through its own drawer row when RPCS3 is added by hand', async () => {
    await $(testId('nav-emulators')).click();
    await $(testId('emulators-view')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators view never rendered',
    });

    await $(testId('emulator-add')).click();
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Name typed FIRST so the form's autofill ("both blank" guard) leaves it
    // alone: the entry has to stay named `RPCS3`, which is what the
    // `emulator-ps3-firmware-rpcs3` testids below are derived from.
    await $(testId('emu-form-name')).setValue('RPCS3');
    await $(testId('emu-form-path')).setValue(rpcs3Exe());
    await $(testId('emu-form-save')).click();
    await $(testId('emulator-row-rpcs3')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the RPCS3 entry never appeared in the emulator list',
    });

    // `spawn_ps3_firmware` admits its own external drawer row, titled
    // verbatim "PS3 Firmware" (firmware_service.rs's PS3_FIRMWARE_TITLE).
    await $(testId('download-row-2')).waitForExist({
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: 'adding RPCS3 never admitted a PS3 firmware drawer row',
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
    // The note and the button only render once `rpcs3_firmware_status`
    // reports a PUP; the panel re-queries it when a `firmware`-kind drawer
    // entry completes.
    await $(testId('emulator-ps3-firmware-note-rpcs3')).waitForExist({
      timeout: FIRMWARE_TIMEOUT,
      timeoutMsg: 'the RPCS3 card never showed the downloaded-firmware note',
    });
    await $(testId('emulator-ps3-firmware-rpcs3')).click();

    await $(testId('emulator-ps3-firmware-toast')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clicking Install PS3 Firmware never produced a toast',
    });
    await expect($(testId('emulator-ps3-firmware-toast'))).toHaveText(
      'PS3 firmware installation started — follow the RPCS3 dialog to complete.',
    );

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
