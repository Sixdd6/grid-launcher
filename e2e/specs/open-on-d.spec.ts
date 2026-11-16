import { APP_START_TIMEOUT } from '../helpers/env.js';
import { expectStartView } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part D: forced Library with the server answering opens on
 * All games although RomM is connected.
 */
describe('open-on (d): forced Library while connected', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared on relaunch',
    });
  });

  it('opens on All games, connected', async () => {
    await expectStartView('all');
    // RomM connects in the background: Discover and the platforms arrive,
    // and the selection stays on All games.
    await $(testId('library-rail-romm-discover')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'RomM never connected on relaunch',
    });
    await $(testId('library-rail-platform-snes')).waitForExist({ timeout: APP_START_TIMEOUT });
    await expect($(testId('library-rail-romm-installed-flag'))).not.toExist();
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
  });
});
