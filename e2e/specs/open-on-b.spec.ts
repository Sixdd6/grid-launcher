import { APP_START_TIMEOUT } from '../helpers/env.js';
import { chooseOpenOn, expectStartView, openAppearance } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part B: a relaunch with the stored session and the server
 * unreachable. Automatic opens on the Library. It then forces Server ›
 * Discover for part C.
 */
describe('open-on (b): Automatic and unreachable opens the Library', () => {
  before(async () => {
    await $(testId('session-chip')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared on relaunch',
    });
  });

  it('opens on the Library, not connected', async () => {
    await expectStartView('library');
    await expect($(testId('session-chip'))).toHaveText('Not connected');
  });

  it('forces Server › Discover for the next start', async () => {
    await openAppearance();
    await chooseOpenOn('discover');
  });
});
