import { APP_START_TIMEOUT, FIXTURE_USERNAME } from '../helpers/env.js';
import { expectStartView } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part D: forced Library with the server answering opens on
 * the Library although GRID is connected.
 */
describe('open-on (d): forced Library while connected', () => {
  before(async () => {
    await $(testId('session-chip')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared on relaunch',
    });
  });

  it('opens on the Library, connected', async () => {
    await expectStartView('library');
    await expect($(testId('session-chip'))).toHaveText(expect.stringContaining(FIXTURE_USERNAME));
  });
});
