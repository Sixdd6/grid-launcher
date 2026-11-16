import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  LOGIN_TIMEOUT,
  loginWindowOpen,
  MAIN_WINDOW,
  submitLogin,
  waitLoginClosed,
  waitLoginForm,
} from '../helpers/gogLogin.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `gog-welcome` (romm-storefront-plan Step 5): the welcome's GOG row on
 * a fresh profile (seed/gog-seed.mjs writes no config for this spec), with
 * the mock GOG. Connect… opens the store login window over the welcome; a
 * sign-in there turns the row to "Signed in as sam_plays" with a Connected
 * pill and a notification, Skip for now goes, and Continue opens the shell
 * on the Library with no emulator step (no RomM, plan 0.11).
 */

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'welcome');

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.pause(250);
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

describe('gog-welcome: a store sign-in from the welcome', () => {
  const library = path.join(dataDir(), 'library');

  before(async () => {
    await $(testId('welcome')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the welcome never appeared on a fresh profile',
    });
    await $(testId('welcome-library-path')).setValue(library);
  });

  it('opens the login window over the welcome and signs in', async () => {
    await $(testId('welcome-connect-gog')).click();
    await browser.waitUntil(loginWindowOpen, { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the login window never opened' });
    await waitLoginForm();
    await submitLogin('sam_plays');
    await waitLoginClosed();
    await browser.switchToWindow(MAIN_WINDOW);
    await waitForToast('Connected to GOG as sam_plays', { timeout: LOGIN_TIMEOUT });
    await browser.waitUntil(async () => (await $(testId('welcome-row-sub-gog')).getText()) === 'Signed in as sam_plays', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the GOG row never showed the account',
    });
    await expect($(testId('welcome-row-pill-gog'))).toHaveText('Connected');
    await expect($(testId('welcome-connect-gog'))).not.toExist();
    await shot('8-gog-connected');
  });

  it('turns the footer to Continue alone', async () => {
    await expect($(testId('welcome-skip'))).not.toExist();
    await expect($(testId('welcome-continue'))).toBeEnabled();
    await expect($(testId('welcome-help'))).toHaveText('GRID opens on your Library.');
  });

  it('Continue opens the Library with no emulator step', async () => {
    await $(testId('welcome-continue')).click();
    await $(testId('shell-topbar')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the shell never opened' });
    await expect($(testId('setup-view'))).not.toExist();
    await expect($(testId('welcome'))).not.toExist();
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`library_path = "${library}"`);
    expect(text).toMatch(/\[setup\][^[]*welcomed = true/);
  });
});
