import { APP_START_TIMEOUT, mockUrl } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `connect-restore`, part B: a second launch of the same binary against
 * the data directory part A left behind.
 *
 * Nothing here types a credential. The library may only appear because
 * `restore_session` read server_url/username from config.toml and the token
 * from the OS keyring.
 */
describe('connect-restore (b): relaunch', () => {
  it('restores the session without re-entering credentials', async () => {
    await $(testId('platform-btn-1')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the library never appeared — the stored session was not restored',
    });
    await expect($(testId('connect-submit'))).not.toExist();
    await expect($(testId('connect-secret'))).not.toExist();
  });

  // Last on purpose: part C starts against a server that has revoked the
  // stored token (Q6). The mock outlives this app process, as in `images`.
  it('revokes the stored token on the mock for part C', async () => {
    const res = await fetch(`${mockUrl()}/__e2e__/revoke`, {
      method: 'POST',
      body: JSON.stringify({ revoked: true }),
    });
    expect(res.ok).toBe(true);
    expect(await res.json()).toEqual({ revoked: true });
  });
});
