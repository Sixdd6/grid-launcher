import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openRommPlatform } from '../helpers/romm.js';
import { dismissToasts } from '../helpers/toasts.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const EXPIRED_TITLE = 'RomM sign-in expired';

async function setRevoked(revoked: boolean) {
  const res = await fetch(`${mockUrl()}/__e2e__/revoke`, {
    method: 'POST',
    body: JSON.stringify({ revoked }),
  });
  expect(res.ok).toBe(true);
  expect(await res.json()).toEqual({ revoked });
}

/** Waits for the RomM group's flag to read Sign-in expired. */
async function waitExpiredFlag(timeout = APP_START_TIMEOUT): Promise<void> {
  await browser.waitUntil(
    async () => (await $(testId('library-rail-romm-installed-flag')).getAttribute('title').catch(() => null)) === EXPIRED_TITLE,
    { timeout, timeoutMsg: 'the RomM group never flagged Sign-in expired' },
  );
  await expect($(testId('library-rail-romm-installed-flag'))).toHaveText('!');
}

/**
 * Stage `connect-restore`, part C (Q6; romm-storefront-plan decision 6): a
 * third launch against the data directory parts A and B left behind, with
 * the mock answering 401 to the stored token (part B's last step revoked it).
 *
 * A rejected token is not "can't reach": GRID opens its shell (no welcome,
 * no Connect screen), and RomM shows Sign-in expired on its Library group
 * (the `!` flag) and RomM › Installed's banner, whose Sign in again… asks
 * for a new token. Then the same thing mid-session: a 401 on a live session
 * turns RomM to Sign-in expired and the shell stays.
 */
describe('connect-restore (c): a rejected token shows Sign-in expired', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Library never appeared on relaunch with a rejected token',
    });
  });

  it('starts in the shell with RomM flagged Sign-in expired, and asks for nothing', async () => {
    await waitExpiredFlag();
    await expect($(testId('welcome'))).not.toExist();
    await expect($(testId('romm-signin-dialog'))).not.toExist();
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('library-rail-platform-snes'))).not.toExist();
    // GRID opens on All games whatever RomM's state (Discover removed, 2026-11-17).
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
  });

  it('banners RomM › Installed with RomM sign-in expired · Sign in again…', async () => {
    await $(testId('library-rail-romm-installed')).click();
    const banner = $(testId('romm-banner'));
    await banner.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect(banner).toHaveAttribute('data-kind', 'expired');
    await expect(banner).toHaveText(
      expect.stringContaining('RomM sign-in expired · Sign in again to browse, install and sync saves. Installed games still play.'),
    );
    await expect($(testId('romm-banner-action'))).toHaveText('Sign in again…');
  });

  it('Sign in again… reconnects with a token the server accepts', async () => {
    await setRevoked(false);
    await $(testId('romm-banner-action')).click();
    await $(testId('romm-signin-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    // The stored server is kept; only the token is asked for again.
    await expect($(testId('romm-signin-url'))).toHaveValue(mockUrl());
    await expect($(testId('romm-signin-secret'))).toHaveValue('');
    await $(testId('romm-signin-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('romm-signin-submit')).click();
    await $(testId('romm-signin-dialog')).waitForExist({
      reverse: true,
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the sign-in dialog never closed after a good token',
    });
    await $(testId('library-rail-platform-snes')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the RomM platform rows never came back after signing in again',
    });
    await expect($(testId('romm-banner'))).not.toExist();
    await expect($(testId('library-rail-romm-installed-flag'))).not.toExist();
    await dismissToasts();
  });

  it('shows Sign-in expired and keeps the shell when the server rejects the token mid-session', async () => {
    await openRommPlatform('snes');
    await setRevoked(true);
    // Any request on the live session now gets a 401; this one lists games.
    await $(testId('library-rail-platform-arcade')).click();

    await waitExpiredFlag(TRANSITION_TIMEOUT * 2);
    await expect($(testId('shell-topbar'))).toBeDisplayed();
    await expect($(testId('welcome'))).not.toExist();
    await expect($(testId('romm-signin-dialog'))).not.toExist();
    await $(testId('library-rail-platform-arcade')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
  });

  after(async () => {
    await setRevoked(false);
  });
});
