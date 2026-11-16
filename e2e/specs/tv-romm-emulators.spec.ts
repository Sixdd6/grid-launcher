// e2e-app-args: --tv
import path from 'node:path';
import { existsSync, mkdirSync } from 'node:fs';
import { APP_START_TIMEOUT, dataDir, INSTALL_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { tvRailKeys, tvRailTo } from '../helpers/romm.js';
import { focused, hints, key, waitFocused, waitTab } from '../helpers/tv.js';
import { notices, tapK, waitNotice } from '../helpers/tvBackups.js';
import { testId, text } from '../helpers/tvDetails.js';

const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'romm-emulators');

/** A screenshot for the design parity check (ctr-out/ is git-ignored). */
async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/**
 * Stage `server-emulator`, part C (TVRommStorefront README "RomM ›
 * Emulators"): the same Emulators row and view in TV mode, a --tv launch on
 * the profile parts A and B left connected with nothing installed from the
 * Emulators platform. The row sits by name among the platforms with its
 * count; its list has the EMULATORS and OTHER FILES rows and the Other
 * builds row that opens a page; A installs, X uninstalls behind a confirm.
 */
describe('server-emulator (c): TV Library › RomM › Emulators', () => {
  const pcsx2 = 're-ps2-pcsx2';
  const ares = 're-f804-8041';
  const aresFile = () => path.join(dataDir(), 'library', 'games', 'Emulators', 'multi', 'Ares Nightly (v141) (arch-linux-x64).AppImage');

  async function rowState(tid: string): Promise<string | null> {
    return $(testId(tid)).getAttribute('data-state');
  }

  async function waitState(tid: string, state: string): Promise<void> {
    let last: string | null = null;
    await browser
      .waitUntil(
        async () => {
          last = await rowState(tid);
          return last === state;
        },
        { timeout: INSTALL_TIMEOUT, interval: 150 },
      )
      .catch(() => {
        throw new Error(`${tid} never reached ${state} (last: ${last})`);
      });
  }

  before(async () => {
    await $(testId('tv-tabs')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed its shell' });
    await waitTab('library');
  });

  it('shows the Emulators row by name among the platform rows, with its count', async () => {
    await browser.waitUntil(async () => (await tvRailKeys()).includes('r-platform:emulators'), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the TV rail never showed the Emulators row',
    });
    const rows = (await tvRailKeys()).filter((k) => k.startsWith('r-romm:') || k.startsWith('r-platform:'));
    expect(rows).toEqual(['r-romm:installed', 'r-romm:discover', 'r-platform:emulators', 'r-platform:ps2']);
    await browser.waitUntil(async () => (await text('tv-lib-rail-platform-emulators')).includes('3'), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Emulators row never counted 3',
    });
  });

  it('its view: the header, the counts, Builds for, and Y Search but no LT or RT', async () => {
    await tvRailTo('r-platform:emulators');
    await $(testId('tv-romm-emulators-header')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await text('tv-lib-count')) === '1 emulator · 2 other files · 0 installed', {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the TV counts line never read "1 emulator · 2 other files · 0 installed"',
    });
    expect(await text('tv-romm-emu-host')).toBe('Builds for Linux x64');
    const h = await hints();
    expect(h).toContain('Y Search');
    expect(h.some((x) => x.startsWith('LT') || x.startsWith('RT'))).toBe(false);
    expect(await text('tv-romm-emu-pcsx2')).toContain('Sony PlayStation 2 · v2.0.2');
    expect(await text('tv-romm-emu-file-8041')).toContain('GRID doesn’t know an emulator called “Ares Nightly”');
    expect(await text('tv-romm-emulators')).toContain('Not for this computer');
    await shot('tv-list');
  });

  it('A on an emulator installs it; then A says Open in Systems and X Uninstall', async () => {
    await tapK(pcsx2);
    await waitState('tv-romm-emu-pcsx2', 'installed');
    await waitNotice('PCSX2 v2.0.2 installed from RomM | Set up for Sony PlayStation 2 in Settings › Systems', INSTALL_TIMEOUT);
    await waitFocused(pcsx2);
    await browser.waitUntil(async () => (await hints()).includes('A Open in Systems') && (await hints()).includes('X Uninstall'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the installed row's hints never read A Open in Systems, X Uninstall (${JSON.stringify(await hints())})`,
    });
    await shot('tv-installed');
  });

  it('X asks before it uninstalls the emulator', async () => {
    await key('x');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(`${testId('tv-confirm')} h2`).getText()).toBe('Uninstall PCSX2 v2.0.2?');
    await tapK('dlg-ok');
    await waitState('tv-romm-emu-pcsx2', 'none');
    await waitNotice('PCSX2 uninstalled');
  });

  it('A on an Other file installs it as a plain file; A again asks to uninstall it', async () => {
    await tapK(ares);
    await waitState('tv-romm-emu-file-8041', 'installed');
    await waitNotice('Ares Nightly (v141) (arch-linux-x64).AppImage installed | In your library’s Emulators folder', INSTALL_TIMEOUT);
    expect(existsSync(aresFile())).toBe(true);
    await tapK(ares);
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(`${testId('tv-confirm')} h2`).getText()).toBe('Uninstall Ares Nightly (v141) (arch-linux-x64).AppImage?');
    await tapK('dlg-ok');
    await waitState('tv-romm-emu-file-8041', 'none');
    expect(existsSync(aresFile())).toBe(false);
  });

  it('the Other builds row opens a page of the builds GRID doesn’t install here; their A says why', async () => {
    expect(await text('tv-romm-emu-builds')).toContain('Other builds');
    await tapK('re-builds');
    await $(testId('tv-romm-other-builds')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const page = await text('tv-romm-other-builds');
    expect(page).toContain('PCSX2 (v2.0.2) (arch-windows-x64).zip');
    expect(page).toContain('For Windows x64');
    expect(page).toContain('For Linux 32-bit ARM');
    await shot('tv-other-builds');
    const before = (await notices()).length;
    await tapK('tb-b0');
    await browser.waitUntil(async () => (await notices()).length > before, { timeout: TRANSITION_TIMEOUT });
    const all = await notices();
    expect(all[all.length - 1]).toMatch(/GRID installs builds for Linux x64 on this computer\.$/);
    await key('Escape');
    await $(testId('tv-romm-other-builds')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    expect(await focused()).not.toBeNull();
  });
});
