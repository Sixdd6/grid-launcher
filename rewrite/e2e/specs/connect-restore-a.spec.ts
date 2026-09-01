import {
  APP_START_TIMEOUT,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `connect-restore`, part A: connect once, then let the app exit.
 *
 * Part B reruns the same binary against the SAME data directory and the same
 * mock server, so it sees the config.toml and keyring entry this spec leaves
 * behind. The pair exists because the embedded WebDriver provider cannot
 * restart the app inside a single run — the runner does it instead.
 */
describe('connect-restore (a): first launch', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
  });

  it('connects and reaches the library', async () => {
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();

    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after a successful connect',
    });
  });
});
