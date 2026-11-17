import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `romm-library`, part A (romm-storefront-plan Step 3): RomM never
 * connected and no RomM game installed. The Library rail has no RomM group,
 * and the top bar has three views and no server chip.
 */
describe('romm-library (a): never connected, nothing installed', () => {
  before(async () => {
    // A fresh profile starts on the welcome; Skip for now opens the shell
    // with the suggested library folder and nothing connected.
    await browser.waitUntil(
      async () => (await $(testId('welcome-skip')).isExisting()) && (await $(testId('welcome-library-path')).getValue()) !== '',
      { timeout: APP_START_TIMEOUT, timeoutMsg: 'the welcome never offered Skip for now with a folder' },
    );
    await $(testId('welcome-skip')).waitForClickable({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Skip for now never became clickable',
    });
    await $(testId('welcome-skip')).click();
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
    // No PLATFORMS section: neither a plain heading nor a folding group.
    const headings = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="library-rail"] .rail-heading, [data-testid="library-rail"] .group-name')].map((h) => h.textContent?.trim()),
    );
    expect(headings).not.toContain('PLATFORMS');
  });
});
