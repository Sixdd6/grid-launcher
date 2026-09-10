import { APP_START_TIMEOUT } from '../helpers/env.js';

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
});
