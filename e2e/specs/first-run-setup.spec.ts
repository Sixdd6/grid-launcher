import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Two catalog installs from the mock forge, one after the other. */
const SETUP_TIMEOUT = 40_000;

/** With E2E_SCREENSHOTS=1, the picker and the progress card are saved under
 *  ctr-out/first-run-setup/ (git-ignored) for a look at the real screens. */
async function screenshot(name: string): Promise<void> {
  if (!process.env.E2E_SCREENSHOTS) return;
  const dir = path.resolve(process.cwd(), '..', 'ctr-out', 'first-run-setup');
  mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(path.join(dir, `${name}.png`));
}

/**
 * Stage `first-run-setup`: a fresh profile connects, Connect asks for a
 * library folder, and the platform picker follows.
 *
 * Fixtures (fixtures-first-run-setup): PS2 and Dreamcast, whose standalone
 * emulators the mock forge serves (PCSX2, Redream), and an Emulators platform
 * hosting a PCSX2 package, which PS2 also offers; Zeebo, which nothing
 * installable serves; and SNES, which only RetroArch serves (with a core).
 * RetroArch's two-archive install has no mock forge route, so SNES is shown
 * but not ticked: its rules are covered by grid-core's launch::setup tests.
 */
describe('first-run setup: pick platforms, install their emulators', () => {
  const library = path.join(dataDir(), 'library');

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-library-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('connect-library-path')).setValue(library);
    await $(testId('connect-submit')).click();
  });

  it('shows the picker after connecting, with what each platform offers', async () => {
    await $(testId('setup-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the platform picker never appeared after connecting',
    });
    await $(testId('setup-row-ps2')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Alphabetical by name.
    const order = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid^="setup-row-"]')).map((r) =>
        (r as HTMLElement).dataset.testid!.replace('setup-row-', ''),
      ),
    );
    // The Emulators platform holds emulators, not games: never a row.
    expect(order).toEqual(['dc', 'ps2', 'snes', 'zeebo']);
    await expect($(testId('setup-unavailable-zeebo'))).toExist();
    await expect($(testId('setup-check-zeebo'))).toBeDisabled();
    // Nothing ticked yet, so nothing to install.
    await expect($(testId('setup-install'))).toBeDisabled();
    await screenshot('1-picker');
  });

  it('pre-selects the recommended emulator for a ticked platform', async () => {
    await $(testId('setup-check-ps2')).click();
    await $(testId('setup-check-dc')).click();
    // The catalog download stays recommended; the server's own PCSX2 package
    // is the alternative, and is what this run installs.
    await expect($(testId('setup-emulator-ps2'))).toHaveValue('PCSX2/pcsx2');
    const ps2Options = await browser.execute(() =>
      Array.from((document.querySelector('[data-testid="setup-emulator-ps2"]') as HTMLSelectElement).options).map(
        (o) => `${o.value}=${o.textContent?.trim()}`,
      ),
    );
    expect(ps2Options.slice(0, 2)).toEqual(['PCSX2/pcsx2=PCSX2', 'romm:505=PCSX2 · from your server']);
    // A real selection fires `change`; WebKitGTK's WebDriver select does not,
    // so make the same choice the way the browser reports it.
    await browser.execute(() => {
      const select = document.querySelector('[data-testid="setup-emulator-ps2"]') as HTMLSelectElement;
      select.value = 'romm:505';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await expect($(testId('setup-emulator-ps2'))).toHaveValue('romm:505');
    // SNES offers RetroArch only (with its core). Ticked to read the select,
    // then unticked: the mock forge cannot serve RetroArch.
    await $(testId('setup-check-snes')).click();
    await $(testId('setup-emulator-snes')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const snesOptions = await browser.execute(() =>
      Array.from((document.querySelector('[data-testid="setup-emulator-snes"]') as HTMLSelectElement).options).map(
        (o) => o.textContent?.trim() ?? '',
      ),
    );
    await $(testId('setup-check-snes')).click();
    await $(testId('setup-emulator-snes')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    expect(snesOptions).toHaveLength(1);
    expect(snesOptions[0]).toMatch(/^RetroArch · \S+ core$/);
    await expect($(testId('setup-install'))).toHaveText('Install 2 emulators');
    await screenshot('2-picked');
  });

  it('lands in the app while the emulators install, then sets them as defaults', async () => {
    await $(testId('setup-install')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the shell never opened after starting setup',
    });
    await $(testId('setup-progress')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await screenshot('3-installing');

    let title = '';
    await browser
      .waitUntil(
        async () => {
          title = await $(testId('setup-progress-title')).getText();
          return title === '2 emulators installed';
        },
        { timeout: SETUP_TIMEOUT },
      )
      .catch(() => undefined);
    if (title !== '2 emulators installed') {
      // What the card and the Downloads rows said, for the failure report.
      const rows = await browser.execute(() =>
        Array.from(document.querySelectorAll('[data-testid^="download-detail-"]')).map(
          (d) => `${(d as HTMLElement).dataset.testid}: ${d.textContent?.trim()}`,
        ),
      );
      console.log(`setup card: ${title}\ndownload rows:\n${rows.join('\n')}`);
    }
    expect(title).toBe('2 emulators installed');
    await screenshot('4-done');

    // The chosen defaults land once each entry exists (the install hook).
    await browser.waitUntil(
      () => {
        const config = readFileSync(configPath(), 'utf-8');
        return (
          config.includes('"Sony PlayStation 2" = "PCSX2 (Playstation 2)"') &&
          config.includes('"Sega Dreamcast" = "Redream (Sega Dreamcast)"')
        );
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the chosen defaults never reached config.toml' },
    );
    // The Server view stayed in front the whole time: its header must pick
    // the new default up without being left and re-entered. Selecting a
    // platform in the rail re-reads nothing, so Dreamcast's chip can only show
    // Redream if the view refreshed its defaults while it stayed open.
    await $(testId('platform-btn-2')).click();
    await browser.waitUntil(
      async () => (await $(testId('server-emulator-chip')).getText()).includes('Redream'),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the Server header never showed Dreamcast's new default" },
    );
    const config = readFileSync(configPath(), 'utf-8');
    // PCSX2 came from the server's package (installed like a game, under
    // games/Emulators), not from the catalog download.
    expect(config).toMatch(/path = ".*games\/Emulators\/[^"]*pcsx2[^"]*"/i);
    expect(config).toMatch(/\[setup\]\s*\ncompleted = true/);
    expect(config).not.toContain('[setup.pending');

    await $(testId('setup-progress-dismiss')).click();
    await expect($(testId('setup-progress'))).not.toExist();
  });

  it('opens the picker again from Emulators › Platform defaults', async () => {
    await $(testId('nav-emulators')).click();
    await $(testId('emu-nav-defaults')).click();
    await $(testId('emu-setup-platforms')).click();
    await $(testId('emu-setup-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('setup-installed-ps2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'PCSX2 never showed as installed in the reopened picker',
    });
    await $(testId('setup-skip')).click();
    await expect($(testId('emu-setup-dialog'))).not.toExist();
  });
});
