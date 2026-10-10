import {
  APP_START_TIMEOUT,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const REJECTED_TOKEN_REASON = 'The server rejected your token. Enter a new one.';

async function setRevoked(revoked: boolean) {
  const res = await fetch(`${mockUrl()}/__e2e__/revoke`, {
    method: 'POST',
    body: JSON.stringify({ revoked }),
  });
  expect(res.ok).toBe(true);
  expect(await res.json()).toEqual({ revoked });
}

/**
 * Stage `connect-restore`, part C (Q6): a third launch against the data
 * directory parts A and B left behind, with the mock answering 401 to the
 * stored token (part B's last step revoked it).
 *
 * A rejected token is not "offline": the app must open the Connect form with
 * the server URL filled in, the token field empty and a reason line — not
 * the shell's "Not connected + Retry", which would resend the dead token.
 * Then the same thing mid-session: a 401 on a live session goes back to
 * Connect too.
 */
describe('connect-restore (c): a rejected token re-opens Connect', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — a rejected token did not re-open Connect',
    });
  });

  it('starts on Connect with the server URL filled, the token empty and a reason', async () => {
    await expect($(testId('session-chip'))).not.toExist();
    await expect($(testId('connect-server-url'))).toHaveValue(mockUrl());
    await expect($(testId('connect-secret'))).toHaveValue('');
    await expect($(testId('connect-use-token'))).toBeSelected();
    await expect($(testId('connect-reason'))).toHaveText(REJECTED_TOKEN_REASON);
    await expect($(testId('connect-error'))).not.toExist();
  });

  it('reconnects with a token the server accepts', async () => {
    await setRevoked(false);
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();

    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after the reconnect',
    });
    await expect($(testId('connect-reason'))).not.toExist();
  });

  it('returns to Connect when the server rejects the token mid-session', async () => {
    await $(testId('platform-btn-2')).waitForClickable({ timeout: TRANSITION_TIMEOUT });
    await setRevoked(true);
    // Any request on the live session now gets a 401; this one lists games.
    await $(testId('platform-btn-2')).click();

    await $(testId('connect-reason')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'a 401 mid-session did not re-open the Connect form',
    });
    await expect($(testId('connect-reason'))).toHaveText(REJECTED_TOKEN_REASON);
    await expect($(testId('connect-server-url'))).toHaveValue(mockUrl());
    await expect($(testId('connect-secret'))).toHaveValue('');
  });

  after(async () => {
    await setRevoked(false);
  });
});
