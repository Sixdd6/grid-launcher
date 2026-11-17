import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'romm-library');

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/**
 * Stage `romm-library`, part D (romm-storefront-plan Step 3): a relaunch
 * after Disconnect. RomM is not connected, its installed games stay: the
 * RomM group shows Installed only with the `off` flag, RomM › Installed has
 * no banner and the "Not connected · Connect…" chip, and the games are
 * still in All games.
 */
describe('romm-library (d): disconnected, games installed', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the Library never appeared after Disconnect',
    });
  });

  it('keeps the installed RomM games in All games', async () => {
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('library-card-101'))).toExist();
    await expect($(testId('library-rail-count-all'))).toHaveText('3');
  });

  it('shows the RomM group with Installed only and the off flag', async () => {
    await expect($(testId('library-rail-romm-installed'))).toExist();
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('library-rail-platform-snes'))).not.toExist();
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveText('off');
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveAttribute('title', 'Not connected');
  });

  it('shows RomM › Installed with no banner and the Connect chip', async () => {
    await $(testId('library-rail-romm-installed')).click();
    await $(testId('library-header')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('romm-banner'))).not.toExist();
    await expect($(testId('romm-installed-chip'))).toHaveText('Not connected · Connect…');
    await expect($(testId('library-card-101'))).toExist();
    await shot('d-installed-off');
  });

  it('has no Open GRID on row on Appearance (hidden with Discover, 2026-11-17)', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-nav-appearance')).click();
    await $(testId('theme-select')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('open-on-select'))).not.toExist();
    await expect($(testId('open-on-romm-off'))).not.toExist();
    await $(testId('nav-library')).click();
  });

  // Display only: the Sign-in expired and Connecting states, which the legacy
  // Connect screen and blocking restore still stand in front of until the
  // welcome (Step 5) lands. A `romm-changed` event drives the group's view.
  async function emitRomm(patch: Record<string, unknown>): Promise<void> {
    await browser.execute(async (p: Record<string, unknown>) => {
      const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> } })
        .__TAURI_INTERNALS__;
      const status = {
        connection: 'not_connected',
        server_url: 'http://romm.e2e.invalid',
        username: 'e2euser',
        auth_kind: 'token',
        last_error: '',
        installed_count: 3,
        in_rail: true,
        busy: false,
        ...p,
      };
      await internals.invoke('plugin:event|emit', { event: 'romm-changed', payload: status });
    }, patch);
  }

  it('draws Sign-in expired: the ! flag and the Sign in again banner', async () => {
    await emitRomm({ connection: 'expired' });
    await $(testId('library-rail-romm-installed')).click();
    const banner = $(testId('romm-banner'));
    await banner.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect(banner).toHaveText(
      expect.stringContaining('RomM sign-in expired · Sign in again to browse, install and sync saves. Installed games still play.'),
    );
    await expect($(testId('romm-banner-action'))).toHaveText('Sign in again…');
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveAttribute('title', 'RomM sign-in expired');
    await shot('d-installed-expired');
  });

  it('draws Connecting: the spinner flag and Installed only (no Discover, 2026-11-17)', async () => {
    await emitRomm({ connection: 'not_connected', busy: true });
    await browser.waitUntil(async () => (await $(testId('library-rail-romm-installed-flag')).getAttribute('title').catch(() => null)) === 'Connecting to RomM', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the RomM group never showed Connecting',
    });
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('romm-installed-chip'))).toHaveText('Connecting to romm.e2e.invalid…');
    await shot('d-installed-connecting');
  });

  it('keeps RomM › Installed selected when the startup probe ends', async () => {
    await emitRomm({ connection: 'not_connected', busy: false });
    await browser.waitUntil(async () => (await $(testId('library-rail-romm-installed-flag')).getText().catch(() => '')) === 'off', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the RomM group never showed the off flag again',
    });
    await expect($(testId('library-rail-romm-installed'))).toHaveAttribute('aria-current', 'page');
  });
});
