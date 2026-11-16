import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, FIXTURE_TOKEN, FIXTURE_USERNAME, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { dismissToasts, waitForToast } from '../helpers/toasts.js';

/**
 * Stage `settings-romm` (romm-storefront-plan Step 4): the RomM card on
 * Settings › Game stores, which replaced Settings › Connection, and the
 * needs-RomM panels on Cloud saves and RetroAchievements. The seed opens the
 * shell with RomM never connected and nothing installed, so the first
 * connect from the card needs setup: Choose your systems opens, and Skip
 * comes back to Game stores (decision 9). Then Edit…, Reconnect, Can't
 * reach and Retry, a 401 mid-session (Sign-in expired, the shell stays) and
 * Sign in again…, and Disconnect with its confirm.
 *
 * Each state is also saved as a screenshot under ctr-out/settings-romm/
 * (git-ignored), for the side by side with the RommStorefront prototype.
 */

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'settings-romm');
const HOST = () => new URL(mockUrl()).host;

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.pause(250);
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

async function mock(route: 'offline' | 'revoke', value: boolean): Promise<void> {
  const key = route === 'offline' ? 'offline' : 'revoked';
  const res = await fetch(`${mockUrl()}/__e2e__/${route}`, { method: 'POST', body: JSON.stringify({ [key]: value }) });
  expect(res.ok).toBe(true);
}

async function page(p: string): Promise<void> {
  await $(testId(`settings-nav-${p}`)).click();
  await $(testId(`settings-page-${p}`)).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
}

async function waitText(id: string, text: string, timeout = TRANSITION_TIMEOUT): Promise<void> {
  await browser.waitUntil(async () => (await $(testId(id)).getText()).trim() === text, {
    timeout,
    timeoutMsg: `${id} never read "${text}" (it read "${await $(testId(id))
      .getText()
      .catch(() => '<missing>')}")`,
  });
}

/** The storefront cards' ids in page order. */
async function cardOrder(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid^="store-card-"]'))
      .map((el) => el.getAttribute('data-testid') ?? '')
      .filter((id) => /^store-card-[a-z]+$/.test(id)),
  );
}

async function fillSignIn(url: string | null, secret: string): Promise<void> {
  if (url !== null) await $(testId('romm-signin-url')).setValue(url);
  await $(testId('romm-signin-secret')).setValue(secret);
}

describe('settings-romm: the RomM card on Settings › Game stores', () => {
  before(async () => {
    await $(testId('nav-settings')).waitForClickable({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared',
    });
    await $(testId('nav-settings')).click();
    await page('stores');
  });

  it('renames the pane Game stores, drops Connection and sorts the cards by name', async () => {
    await expect($(testId('settings-nav-connection'))).not.toExist();
    await expect($(testId('settings-page-connection'))).not.toExist();
    await expect($(testId('settings-nav-stores'))).toHaveText('Game stores');
    await expect($(`${testId('settings-page-stores')} h2`)).toHaveText('Game stores');
    await $(testId('store-card-romm')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await cardOrder()).length === 5, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the five storefront cards never rendered',
    });
    expect(await cardOrder()).toEqual(['store-card-amazon', 'store-card-epic', 'store-card-gog', 'store-card-romm', 'store-card-steam']);
  });

  it('Never connected: "Your own game server", Not connected, Connect RomM… only', async () => {
    await expect($(testId('store-card-account-romm'))).toHaveText('Your own game server');
    await expect($(testId('store-card-pill-romm'))).toHaveText('Not connected');
    await expect($(testId('romm-card-lede'))).toHaveText(
      'Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware.',
    );
    await expect($(testId('romm-card-connect'))).toHaveText('Connect RomM…');
    await expect($(testId('romm-card-open'))).not.toExist();
    await expect($(testId('romm-card-field-server'))).not.toExist();
    await $(testId('store-card-romm')).scrollIntoView();
    await shot('01-card-never');
  });

  it('Cloud saves and RetroAchievements say they need RomM, and keep their forms', async () => {
    await page('cloud-saves');
    await expect($(testId('cloud-settings-needs-romm-title'))).toHaveText('Cloud saves need RomM');
    await expect($(testId('cloud-settings-needs-romm-text'))).toHaveText(
      "RomM isn't connected. GRID keeps your emulator saves and states on your RomM server. These settings apply when it's connected. GOG and Epic games sync with their own store's cloud.",
    );
    await $(testId('cloud-settings-download-on-launch')).waitForEnabled({ timeout: TRANSITION_TIMEOUT });
    await shot('02-cloud-saves-needs-romm');

    await page('retroachievements');
    await expect($(testId('ra-needs-romm-title'))).toHaveText('Achievement progress needs RomM');
    await expect($(testId('ra-needs-romm-text'))).toHaveText(
      "RomM isn't connected. Details › Achievements shows your progress from RomM. Signing in here still turns on achievements in RetroArch, DuckStation and Dolphin.",
    );
    await expect($(testId('ra-username'))).toBeEnabled();
    await expect($(testId('ra-token'))).toBeEnabled();
    await shot('03-ra-needs-romm');

    // Open Game stores leads to the card.
    await $(testId('ra-needs-romm-open-stores')).click();
    await $(testId('settings-page-stores')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  });

  it('a first connect from the card opens Choose your systems; Skip returns to Game stores', async () => {
    await $(testId('romm-card-connect')).click();
    await $(testId('romm-signin-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('romm-signin-title'))).toHaveText('Connect to RomM');
    await expect($(testId('romm-signin-use-token'))).toBeSelected();
    await expect($(testId('romm-signin-username'))).not.toExist();
    await expect($(testId('romm-signin-submit'))).toBeDisabled();
    await shot('04-dialog-connect');

    // Password mode asks for the username.
    await $(testId('romm-signin-use-token')).click();
    await $(testId('romm-signin-username')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('romm-signin-use-token')).click();
    await $(testId('romm-signin-username')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });

    await fillSignIn(mockUrl(), FIXTURE_TOKEN);
    await expect($(testId('romm-signin-submit'))).toBeEnabled();
    await $(testId('romm-signin-submit')).click();

    await $(testId('setup-view')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'Choose your systems never opened after a first connect from the card',
    });
    await waitForToast(`Connected to RomM at ${HOST()} as ${FIXTURE_USERNAME}`);
    await $(testId('setup-skip')).waitForClickable({ timeout: APP_START_TIMEOUT });
    await $(testId('setup-skip')).click();

    await $(testId('settings-page-stores')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'Skip did not return to Settings › Game stores',
    });
    await waitText('store-card-pill-romm', 'Connected');
    await expect($(testId('store-card-account-romm'))).toHaveText(`Signed in as ${FIXTURE_USERNAME} · ${HOST()}`);
    await expect($(testId('romm-card-field-server'))).toHaveText(mockUrl());
    await expect($(testId('romm-card-field-user'))).toHaveText(FIXTURE_USERNAME);
    await expect($(testId('romm-card-field-signin'))).toHaveText('API token · kept in the OS keyring');
    await expect($(testId('romm-card-field-status'))).toHaveText('Connected');
    await browser.waitUntil(async () => /^\d+ on the server · 0 installed here$/.test(await $(testId('romm-card-field-games')).getText()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Games field never showed the server count',
    });
    const actions = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="store-card-romm"] .g-actions button')).map((b) => b.textContent?.trim()),
    );
    expect(actions).toEqual(['Show in Library', 'Edit…', 'Reconnect', 'Open in browser', 'Disconnect…']);
    await dismissToasts();
    await $(testId('store-card-romm')).scrollIntoView();
    await shot('05-card-connected');

    // With RomM connected the panels are gone.
    await page('cloud-saves');
    await expect($(testId('cloud-settings-needs-romm'))).not.toExist();
    await page('stores');
  });

  it('Edit… shows a failed connect in red, then Save and connect works', async () => {
    await $(testId('romm-card-edit')).click();
    await $(testId('romm-signin-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('romm-signin-title'))).toHaveText('Edit RomM sign-in');
    await expect($(testId('romm-signin-url'))).toHaveValue(mockUrl());
    await expect($(testId('romm-signin-secret'))).toHaveValue('');
    await expect($(testId('romm-signin-submit'))).toHaveText('Save and connect');

    const dead = 'http://127.0.0.1:9';
    await fillSignIn(dead, FIXTURE_TOKEN);
    await $(testId('romm-signin-submit')).click();
    await waitText('romm-signin-error', `Couldn't reach ${dead}. Check the address, and that the server is running.`, APP_START_TIMEOUT);
    // The secret is asked for again after a failed connect.
    await expect($(testId('romm-signin-secret'))).toHaveValue('');
    await shot('06-dialog-edit-error');

    await fillSignIn(mockUrl(), FIXTURE_TOKEN);
    await $(testId('romm-signin-submit')).click();
    await $(testId('romm-signin-dialog')).waitForExist({ reverse: true, timeout: APP_START_TIMEOUT });
    await waitForToast(`Connected to RomM at ${HOST()} as ${FIXTURE_USERNAME}`);
    await waitText('store-card-pill-romm', 'Connected');
    await dismissToasts();
  });

  it('Reconnect re-checks the stored sign-in', async () => {
    await $(testId('romm-card-reconnect')).click();
    await waitForToast(`Reconnected to ${HOST()}`, { timeout: APP_START_TIMEOUT });
    await waitText('store-card-pill-romm', 'Connected');
    await dismissToasts();
  });

  it("Can't reach: the warning and Retry; Retry connects again", async () => {
    await mock('offline', true);
    try {
      await $(testId('romm-card-reconnect')).click();
      await waitText('store-card-pill-romm', "Can't reach", APP_START_TIMEOUT);
      await expect($(testId('romm-card-warning'))).toHaveText(expect.stringContaining("Can't reach RomM."));
      await expect($(testId('romm-card-retry'))).toHaveText('Retry');
      await expect($(testId('romm-card-show'))).not.toExist();
      await $(testId('store-card-romm')).scrollIntoView();
      await shot('07-card-unreachable');

      // The needs-RomM panel offers Retry too.
      await page('cloud-saves');
      await expect($(testId('cloud-settings-needs-romm-text'))).toHaveText(expect.stringContaining(`GRID can't reach ${HOST()} right now.`));
      await expect($(testId('cloud-settings-needs-romm-retry'))).toHaveText('Retry');
      await page('stores');
    } finally {
      await mock('offline', false);
    }
    await $(testId('romm-card-retry')).click();
    await waitForToast(`Connected to RomM again · ${HOST()}`, { timeout: APP_START_TIMEOUT });
    await waitText('store-card-pill-romm', 'Connected');
    await dismissToasts();
  });

  it('a 401 mid-session: Sign-in expired, the shell stays, Sign in again… signs in', async () => {
    await mock('revoke', true);
    try {
      // Coming back to the pane reads the server's game count: a 401 now.
      await page('appearance');
      await page('stores');
      await waitText('store-card-pill-romm', 'Sign-in expired', APP_START_TIMEOUT);
      await expect($(testId('nav-settings'))).toExist();
      await expect($(testId('welcome'))).not.toExist();
      await expect($(testId('romm-card-warning'))).toHaveText(
        'Your RomM sign-in expired. Sign in again to browse, install and sync saves. Installed games still play.',
      );
      await expect($(testId('romm-card-field-status'))).toHaveText('Sign-in expired · RomM answered 401');
      await $(testId('store-card-romm')).scrollIntoView();
      await shot('08-card-expired');

      await $(testId('romm-card-signin')).click();
      await $(testId('romm-signin-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
      await expect($(testId('romm-signin-title'))).toHaveText('Sign in to RomM again');
      await expect($(testId('romm-signin-url'))).toBeDisabled();
      await expect($(testId('romm-signin-url'))).toHaveValue(mockUrl());
      await expect($(testId('romm-signin-username'))).toBeDisabled();
      await expect($(testId('romm-signin-username'))).toHaveValue(FIXTURE_USERNAME);
      await expect($(testId('romm-signin-use-token'))).not.toExist();
      await expect($(testId('romm-signin-submit'))).toHaveText('Sign in');
      await shot('09-dialog-again');
    } finally {
      await mock('revoke', false);
    }
    await fillSignIn(null, FIXTURE_TOKEN);
    await $(testId('romm-signin-submit')).click();
    await $(testId('romm-signin-dialog')).waitForExist({ reverse: true, timeout: APP_START_TIMEOUT });
    await waitForToast(`Signed in to RomM again as ${FIXTURE_USERNAME}`);
    await waitText('store-card-pill-romm', 'Connected');
    await dismissToasts();
  });

  it('Disconnect… asks first, stays in the app and keeps the server and user', async () => {
    await $(testId('romm-card-disconnect')).click();
    await expect($(testId('romm-disconnect-confirm'))).toHaveText(
      expect.stringContaining('Disconnect RomM? GRID deletes your RomM sign-in from the OS keyring and stays open.'),
    );
    await $(testId('store-card-romm')).scrollIntoView();
    await shot('10-card-disconnect-confirm');
    await $(testId('romm-disconnect-cancel')).click();
    await expect($(testId('romm-disconnect-confirm'))).not.toExist();
    await waitText('store-card-pill-romm', 'Connected');

    await $(testId('romm-card-disconnect')).click();
    await $(testId('romm-disconnect-go')).click();
    await waitForToast('RomM disconnected. Installed RomM games stay and still play.');
    await waitText('store-card-pill-romm', 'Not connected');
    await expect($(testId('store-card-account-romm'))).toHaveText(`Not signed in · ${HOST()}`);
    await expect($(testId('romm-card-field-server'))).toHaveText(mockUrl());
    await expect($(testId('romm-card-field-user'))).toHaveText(FIXTURE_USERNAME);
    await expect($(testId('romm-card-field-status'))).toHaveText('Not connected');
    await expect($(testId('romm-card-open'))).toHaveText('Open in browser');
    await expect($(testId('nav-settings'))).toExist();
    await expect($(testId('welcome'))).not.toExist();
    await dismissToasts();
    await $(testId('store-card-romm')).scrollIntoView();
    await shot('11-card-not-connected');
  });

  it('Connect RomM… again: filled in, no Choose your systems this time', async () => {
    await $(testId('romm-card-connect')).click();
    await $(testId('romm-signin-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('romm-signin-url'))).toHaveValue(mockUrl());
    await fillSignIn(null, FIXTURE_TOKEN);
    await $(testId('romm-signin-submit')).click();
    await $(testId('romm-signin-dialog')).waitForExist({ reverse: true, timeout: APP_START_TIMEOUT });
    await waitText('store-card-pill-romm', 'Connected');
    await expect($(testId('setup-view'))).not.toExist();
    await expect($(testId('settings-page-stores'))).toBeDisplayed();
  });
});
