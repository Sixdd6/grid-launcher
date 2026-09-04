import {
  APP_START_TIMEOUT,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `library`, second spec: the redesigned rails, toolbars and card
 * chrome (design §5, §6, D-UI-2/3/9). Runs after `library.spec.ts` in the
 * same group, so the app is already connected with the base fixtures —
 * platform 1 (SNES) holds roms 101/102/103, platform 2 (Arcade) holds 201
 * and 301. Nothing is installed in this group, so the Library grid is
 * empty and its empty states are what there is to assert.
 */
describe('library and server chrome', () => {
  /**
   * Sets a native `<select>`'s value and dispatches `change` directly. A
   * WebDriver-protocol click on the underlying `<option>` (what
   * `selectByAttribute` does) does not reliably fire a `change` event on
   * this embedded WebKitGTK driver — the same limitation `emulators.spec.ts`
   * and `launch.spec.ts` already work around. Confirmed here too: the
   * toolbar `<select>` still read `medium` straight after
   * `selectByAttribute('value', 'large')`.
   */
  async function selectValue(testIdName: string, value: string) {
    await browser.execute(
      (selector, val) => {
        const el = document.querySelector(selector) as HTMLSelectElement | null;
        if (!el) throw new Error(`no element matched ${selector}`);
        el.value = val;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      },
      testId(testIdName),
      value,
    );
  }

  before(async () => {
    // The group's first spec already connected, and this process re-uses the
    // data dir, so `restore_session` normally brings the rail straight back
    // and the connect form never renders. Wait for whichever of the two
    // arrives, then only type credentials if the form is the one that did.
    await browser.waitUntil(
      async () =>
        (await $(testId('platform-btn-1')).isExisting()) ||
        (await $(testId('connect-submit')).isExisting()),
      {
        timeout: APP_START_TIMEOUT,
        timeoutMsg:
          'neither the restored Server rail nor the connect form appeared — the app did not reach a usable state',
      },
    );
    if (await $(testId('connect-submit')).isExisting()) {
      await $(testId('connect-server-url')).setValue(mockUrl());
      await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
      await $(testId('connect-submit')).click();
    }
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Server rail never rendered a platform entry after connecting',
    });
  });

  it('gives each Server rail entry both its old test id and its new rail id', async () => {
    // Design §11 adds `server-rail-<id>`; every existing spec still clicks
    // `platform-btn-<id>`. Both live on one element.
    await expect($(testId('platform-btn-1'))).toHaveAttribute('data-rail', 'server-rail-1');
    await expect($(testId('platform-btn-2'))).toHaveAttribute('data-rail', 'server-rail-2');
  });

  it('heads the selected platform with its name and counts', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('server-platform-header'))).toBeDisplayed();
    await expect($(testId('server-platform-counts'))).toHaveText('3 games · 0 installed');
    await expect($(testId('server-firmware-chip'))).toBeDisplayed();
    await expect($(testId('server-emulator-chip'))).toBeDisplayed();
  });

  it('filters the Server grid client-side from its search box', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    await $(testId('server-search')).setValue('chrono');
    await $(testId('game-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'Super Mario World survived a search for "chrono"',
    });
    await expect($(testId('game-card-102'))).toExist();

    await $(testId('server-search')).setValue('');
    await $(testId('game-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clearing the search never restored the full platform list',
    });
  });

  it('Ctrl+F focuses the active view search box', async () => {
    await $(testId('nav-server')).click();
    await browser.keys(['Control', 'f']);
    const focused = await browser.execute(
      () => document.activeElement?.getAttribute('data-testid') ?? '',
    );
    expect(focused).toBe('server-search');

    await $(testId('nav-library')).click();
    await browser.keys(['Control', 'f']);
    const libraryFocused = await browser.execute(
      () => document.activeElement?.getAttribute('data-testid') ?? '',
    );
    expect(libraryFocused).toBe('library-search');
  });

  it('shows the Library rail with its three fixed entries and their counts', async () => {
    await $(testId('nav-library')).click();
    await $(testId('library-rail')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-rail-all'))).toExist();
    await expect($(testId('library-rail-recent'))).toExist();
    await expect($(testId('library-rail-updates'))).toExist();
    await expect($(testId('library-rail-count-all'))).toHaveText('0');
  });

  it('gives each Library rail entry its own empty state, verbatim', async () => {
    await $(testId('nav-library')).click();
    await $(testId('library-rail-all')).click();
    await expect($(testId('library-empty'))).toHaveText('No games installed');

    await $(testId('library-rail-recent')).click();
    await expect($(testId('library-empty'))).toHaveText('Nothing played in the last 30 days');

    await $(testId('library-rail-updates')).click();
    await expect($(testId('library-empty'))).toHaveText('Everything is up to date');

    await $(testId('library-rail-all')).click();
  });

  it('remembers each grid card size across a view switch', async () => {
    await $(testId('nav-server')).click();
    await selectValue('server-size', 'large');
    await $(testId('nav-library')).click();
    await selectValue('library-size', 'small');

    await $(testId('nav-server')).click();
    await expect($(testId('server-size'))).toHaveValue('large');
    await $(testId('nav-library')).click();
    await expect($(testId('library-size'))).toHaveValue('small');
  });
});
