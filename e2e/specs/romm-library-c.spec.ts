import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { setServerOffline } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'romm-library');

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/**
 * Stage `romm-library`, part C (romm-storefront-plan Step 3): a relaunch
 * with RomM stored as connected and the server not answering. GRID opens on
 * All games (Discover and Open GRID on are removed for now, 2026-11-17); the
 * RomM group has Installed only, with the `!` flag, and RomM › Installed
 * shows the Can't reach banner. Retry, once the server is back, brings the
 * platforms back. Then Disconnect on the
 * RomM card: a platform row disappears and the selection falls back.
 */
describe('romm-library (c): Can\'t reach RomM', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Library never appeared on relaunch',
    });
  });

  it('opens on All games, which still holds the installed RomM games', async () => {
    // RomM stored as connected: the startup probe fails, and GRID stays on
    // All games (SPEC "Where GRID opens").
    await browser.waitUntil(async () => (await $(testId('library-rail-romm-installed-flag')).getText().catch(() => '')) === '!', {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the RomM group never showed the ! flag after the failed probe',
    });
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('library-rail-count-all'))).toHaveText('3');
    await expect($(testId('library-card-101'))).toExist();
  });

  it('shows the RomM group with Installed only and the ! flag', async () => {
    await $(testId('library-rail-romm-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('library-rail-platform-snes'))).not.toExist();
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveText('!');
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveAttribute('title', "Can't reach RomM");
  });

  it('banners RomM › Installed with Can\'t reach RomM · Retry, and no chip', async () => {
    await $(testId('library-rail-romm-installed')).click();
    const banner = $(testId('romm-banner'));
    await banner.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const host = new URL(mockUrl()).host;
    await expect(banner).toHaveText(expect.stringContaining(`Can't reach RomM · ${host} didn't answer. Installed games still play.`));
    await expect($(testId('romm-banner-action'))).toHaveText('Retry');
    await expect($(testId('romm-installed-chip'))).not.toExist();
    await shot('c-installed-unreachable');
  });

  it('Retry, with the server back, brings the platforms back', async () => {
    await setServerOffline(false);
    await $(testId('romm-banner-action')).click();
    await $(testId('library-rail-platform-snes')).waitForExist({
      timeout: TRANSITION_TIMEOUT * 2,
      timeoutMsg: 'Retry never reconnected RomM',
    });
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('romm-banner'))).not.toExist();
    await expect($(testId('library-rail-romm-installed-flag'))).not.toExist();
  });

  it('falls back to RomM › Installed when a platform row disappears', async () => {
    await $(testId('library-rail-platform-snes')).click();
    await $(testId('server-platform-header')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    // Disconnect on the RomM card (Settings › Game stores); GRID stays open.
    await $(testId('nav-settings')).click();
    await $(testId('settings-nav-stores')).click();
    await $(testId('romm-card-disconnect')).waitForClickable({ timeout: TRANSITION_TIMEOUT });
    await $(testId('romm-card-disconnect')).click();
    await $(testId('romm-disconnect-go')).click();
    await $(testId('nav-library')).click();
    await browser.waitUntil(async () => (await $(testId('library-rail-romm-installed')).getAttribute('aria-current')) === 'page', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the selection never fell back to RomM › Installed after Disconnect',
    });
    await expect($(testId('library-rail-platform-snes'))).not.toExist();
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveText('off');
  });
});
