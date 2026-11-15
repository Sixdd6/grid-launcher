import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `romm-library`, part A (romm-storefront-plan Step 3): RomM never
 * connected and no RomM game installed. The Library rail has no RomM group,
 * and the top bar has three views and no server chip.
 */
describe('romm-library (a): never connected, nothing installed', () => {
  before(async () => {
    await $(testId('connect-use-offline')).waitForClickable({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Connect screen never offered "Use without a server"',
    });
    await $(testId('connect-use-offline')).click();
    await $(testId('library-rail')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Library never appeared',
    });
  });

  it('shows Library, Downloads and Settings, no Server pill and no server chip', async () => {
    const pills = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="shell-topbar"] nav button')].map((b) => b.textContent?.trim()),
    );
    expect(pills).toEqual(['Library', 'Downloads', 'Settings']);
    await expect($(testId('nav-server'))).not.toExist();
    await expect($(testId('session-chip'))).not.toExist();
    await expect($(testId('session-dot'))).not.toExist();
  });

  it('opens on All games and lists no RomM group', async () => {
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('library-rail-romm-installed'))).not.toExist();
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    // No PLATFORMS section.
    const headings = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="library-rail"] .rail-heading')].map((h) => h.textContent?.trim()),
    );
    expect(headings).not.toContain('PLATFORMS');
  });
});
