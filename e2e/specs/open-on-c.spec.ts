import { APP_START_TIMEOUT } from '../helpers/env.js';
import { chooseOpenOn, expectStartView, openAppearance, setServerOffline } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part C: forced Discover with the server still
 * unreachable. Discover needs RomM: once the startup connect fails, the
 * selection falls back to RomM › Installed, and the Appearance note says
 * RomM isn't connected. It then forces the Library and brings the mock
 * back for part D.
 */
describe('open-on (c): forced Discover with RomM unreachable', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared on relaunch',
    });
  });

  it('falls back to RomM › Installed, with no Discover row', async () => {
    await expectStartView('romm:installed');
    await browser.waitUntil(
      async () => (await $(testId('library-rail-romm-installed-flag')).getAttribute('title').catch(() => null)) === "Can't reach RomM",
      { timeout: APP_START_TIMEOUT, timeoutMsg: "the RomM group never flagged Can't reach" },
    );
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('library-rail-romm-installed'))).toHaveAttribute('aria-current', 'page');
  });

  it('forces the Library for the next start', async () => {
    await openAppearance();
    await expect($(testId('open-on-select'))).toHaveValue('discover');
    await expect($(testId('open-on-romm-off'))).toBeDisplayed();
    await chooseOpenOn('library');
    await expect($(testId('open-on-romm-off'))).not.toExist();
    await setServerOffline(false);
  });
});
