import { APP_START_TIMEOUT } from '../helpers/env.js';
import { chooseOpenOn, expectStartView, openAppearance, setServerOffline } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part C: forced Server › Discover with the server still
 * unreachable opens on the Server view's not-connected panel. It then forces
 * the Library and brings the mock back for part D.
 */
describe('open-on (c): forced Discover with no server', () => {
  before(async () => {
    await $(testId('session-chip')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared on relaunch',
    });
  });

  it("opens on the Server view's not-connected panel", async () => {
    await expectStartView('server');
    await expect($(testId('server-offline'))).toBeDisplayed();
  });

  it('forces the Library for the next start', async () => {
    await openAppearance();
    await expect($(testId('open-on-select'))).toHaveValue('discover');
    await chooseOpenOn('library');
    await setServerOffline(false);
  });
});
