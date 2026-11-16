import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  FIXTURE_USERNAME,
  mockUrl,
  TRANSITION_TIMEOUT,
  WRONG_TOKEN,
} from '../helpers/env.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `welcome` (romm-storefront-plan Step 5; RommStorefront README,
 * "Welcome (replaces Connect)"): one app instance on a fresh data directory.
 * The welcome shows, with the library folder (required) and the storefront
 * rows by name. RomM connects inline (a wrong token first, then the fixture
 * token), Steam's form opens inline, and the footer turns from Skip for now
 * to Continue. Continue stores the folder and `[setup] welcomed`, and opens
 * the shell (no first-run picker: the e2e build only offers it to the groups
 * that set E2E_FIRST_RUN_SETUP).
 *
 * Each state is saved as a screenshot under ctr-out/welcome/ (git-ignored),
 * for the side by side with the RommStorefront prototype.
 */

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'welcome');
const HOST = () => new URL(mockUrl()).host;

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.pause(250);
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

async function rowOrder(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid^="welcome-row-"]'))
      .map((el) => el.getAttribute('data-testid') ?? '')
      .filter((id) => /^welcome-row-[a-z]+$/.test(id))
      .map((id) => id.replace('welcome-row-', '')),
  );
}

async function waitText(id: string, text: string): Promise<void> {
  await browser.waitUntil(async () => (await $(testId(id)).getText()).trim() === text, {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `${id} never read "${text}" (it read "${await $(testId(id))
      .getText()
      .catch(() => '<missing>')}")`,
  });
}

describe('welcome: the first-run screen', () => {
  const library = path.join(dataDir(), 'library');

  before(async () => {
    await $(testId('welcome')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the welcome never appeared on a fresh profile',
    });
  });

  it('shows the folder step and the storefront rows by name', async () => {
    await expect($(`${testId('welcome')} h1`)).toHaveText('Welcome to GRID');
    await expect($(testId('welcome-import-notice'))).not.toExist();
    // The folder starts on the suggested one.
    await browser.waitUntil(async () => (await $(testId('welcome-library-path')).getValue()) !== '', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library folder never started on the suggested folder',
    });
    expect(await rowOrder()).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam']);
    await expect($(testId('welcome-row-sub-gog'))).toHaveText('Sign in on GOG’s own page. GRID never sees your password.');
    await expect($(testId('welcome-row-sub-romm'))).toHaveText(
      'Your own game server. GRID browses it and installs its games with an emulator.',
    );
    await expect($(testId('welcome-row-sub-steam'))).toHaveText('Installed Steam games show as soon as Steam is set up here.');
    await expect($(testId('welcome-connect-amazon'))).toHaveText('Connect…');
    await expect($(testId('welcome-connect-romm'))).toHaveText('Connect…');
    await expect($(testId('welcome-connect-steam'))).toHaveText('Set up…');
    // Nothing connected: Skip for now is the way on; Continue waits.
    await expect($(testId('welcome-skip'))).toBeEnabled();
    await expect($(testId('welcome-continue'))).toBeDisabled();
    await expect($(testId('welcome-help'))).toHaveText('Nothing connected yet. Skip for now opens GRID with your library folder.');
    await shot('1-new-user');
  });

  it('shows the light / dark toggle in the corner', async () => {
    const toggle = $(testId('theme-toggle-start'));
    await expect(toggle).toBeDisplayed();
    const { right, top } = await browser.execute((sel: string) => {
      const r = document.querySelector(sel)!.getBoundingClientRect();
      return { right: window.innerWidth - r.right, top: r.top };
    }, testId('theme-toggle-start'));
    expect(Math.round(right)).toBe(16);
    expect(Math.round(top)).toBe(16);
  });

  it('holds both buttons until a folder is chosen', async () => {
    await $(testId('welcome-library-path')).clearValue();
    await $(testId('welcome-library-missing')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('welcome-library-missing'))).toHaveText('Choose a folder to continue.');
    await expect($(testId('welcome-skip'))).toBeDisabled();
    await expect($(testId('welcome-continue'))).toBeDisabled();
    await shot('2-no-folder');
    await $(testId('welcome-library-path')).setValue(library);
    await expect($(testId('welcome-library-missing'))).not.toExist();
    await expect($(testId('welcome-skip'))).toBeEnabled();
  });

  it('opens Steam’s form inline and closes it', async () => {
    await $(testId('welcome-connect-steam')).click();
    await $(testId('steam-accounts')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('steam-save'))).toBeDisplayed();
    await expect($(testId('steam-test'))).toBeDisplayed();
    // The card's own head is not drawn inside the row.
    await expect($(testId('store-card-pill-steam'))).not.toBeDisplayed();
    await shot('3-steam-form');
    await $(testId('welcome-steam-close')).click();
    await expect($(testId('steam-accounts'))).not.toExist();
  });

  it('opens RomM’s form inline and keeps it open on a rejected token', async () => {
    await $(testId('welcome-connect-romm')).click();
    await $(testId('welcome-romm-url')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('welcome-romm-submit'))).toHaveText('Connect');
    await expect($(testId('welcome-romm-submit'))).toBeDisabled();
    await $(testId('welcome-romm-url')).setValue(mockUrl());
    await $(testId('welcome-romm-secret')).setValue(WRONG_TOKEN);
    await shot('4-romm-form');
    await $(testId('welcome-romm-submit')).click();
    await waitText('welcome-romm-error', 'RomM rejected this token (401). Check it, or make a new one.');
    // The secret is asked for again.
    await expect($(testId('welcome-romm-secret'))).toHaveValue('');
    await expect($(testId('welcome-skip'))).toBeEnabled();
    await shot('5-romm-rejected');
  });

  it('Cancel closes the form', async () => {
    await $(testId('welcome-romm-cancel')).click();
    await expect($(testId('welcome-romm-url'))).not.toExist();
    await expect($(testId('welcome-connect-romm'))).toBeDisplayed();
  });

  it('connects RomM: a connected row with Edit, and Continue alone', async () => {
    await $(testId('welcome-connect-romm')).click();
    await $(testId('welcome-romm-url')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('welcome-romm-url')).setValue(mockUrl());
    await $(testId('welcome-romm-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('welcome-romm-submit')).click();
    await waitForToast(`Connected to RomM at ${HOST()} as ${FIXTURE_USERNAME}`);
    await $(testId('welcome-romm-url')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await waitText('welcome-row-sub-romm', `Connected as ${FIXTURE_USERNAME} · ${HOST()}`);
    await expect($(testId('welcome-row-pill-romm'))).toHaveText('Connected');
    await expect($(testId('welcome-romm-edit'))).toBeDisplayed();
    // Something connected: Continue alone. This profile needs no picker.
    await expect($(testId('welcome-skip'))).not.toExist();
    await expect($(testId('welcome-continue'))).toBeEnabled();
    await expect($(testId('welcome-help'))).toHaveText('GRID opens on your Library.');
    // Still on the welcome: a RomM connect does not jump away.
    await expect($(testId('welcome'))).toBeDisplayed();
    await shot('6-romm-connected');
  });

  it('Edit opens the form filled in', async () => {
    await $(testId('welcome-romm-edit')).click();
    await $(testId('welcome-romm-url')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('welcome-romm-url'))).toHaveValue(mockUrl());
    await shot('7-romm-edit');
    await $(testId('welcome-romm-cancel')).click();
    await expect($(testId('welcome-romm-url'))).not.toExist();
  });

  it('keeps the token out of the DOM and config.toml', async () => {
    const found = await browser.execute((token: string) => {
      const markup = document.documentElement.outerHTML;
      const values = Array.from(document.querySelectorAll('input')).map((el) => el.value);
      return markup.includes(token) || values.some((v) => v.includes(token));
    }, FIXTURE_TOKEN);
    expect(found).toBe(false);
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`server_url = "${mockUrl()}"`);
    expect(text).toContain(`username = "${FIXTURE_USERNAME}"`);
    expect(text).not.toContain(FIXTURE_TOKEN);
    expect(text).not.toContain('token');
    expect(text).not.toContain('password');
  });

  it('Continue stores the folder and the welcome, and opens the shell', async () => {
    await $(testId('welcome-continue')).click();
    await $(testId('shell-topbar')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the shell never opened after Continue',
    });
    await expect($(testId('welcome'))).not.toExist();
    await expect($(testId('theme-toggle-start'))).not.toExist();
    await expect($(testId('shell-topbar')).$(testId('theme-toggle'))).toBeDisplayed();
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`library_path = "${library}"`);
    expect(text).toMatch(/\[setup\][^[]*welcomed = true/);
    expect(text).toMatch(/\[stores\.romm\][^[]*connected = true/);
  });
});
