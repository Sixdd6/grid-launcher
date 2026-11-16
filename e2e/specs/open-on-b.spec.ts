import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { chooseOpenOn, expectStartView, openAppearance } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part B: a relaunch with RomM stored as connected and the
 * server unreachable. Automatic opens on Library › RomM › Discover while
 * RomM is connecting at startup; when the server doesn't answer, Discover
 * goes and the selection falls back to RomM › Installed, and RomM's Library
 * group says Can't reach (romm-storefront-plan decision 6: no chip). It then
 * forces Discover for part C.
 */
describe('open-on (b): Automatic and unreachable falls back to RomM › Installed', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared on relaunch',
    });
  });

  it('falls back to RomM › Installed, with RomM flagged Can\'t reach', async () => {
    await expectStartView('romm:installed');
    await browser.waitUntil(
      async () => (await $(testId('library-rail-romm-installed-flag')).getAttribute('title').catch(() => null)) === "Can't reach RomM",
      { timeout: APP_START_TIMEOUT, timeoutMsg: "the RomM group never flagged Can't reach" },
    );
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('library-rail-romm-installed'))).toHaveAttribute('aria-current', 'page');
  });

  it('forces Discover for the next start, and says RomM isn\'t connected', async () => {
    await openAppearance();
    await chooseOpenOn('discover');
    await $(testId('open-on-romm-off')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('open-on-romm-off'))).toHaveText("RomM isn't connected, so GRID opens on All games.");
  });
});
