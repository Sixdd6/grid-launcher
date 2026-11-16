import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openRommPlatform } from '../helpers/romm.js';

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
    // data dir, so the shell opens at once and RomM's restore brings the
    // platform rows back.
    await openRommPlatform('snes');
  });

  it('heads the selected platform with its name and counts', async () => {
    await openRommPlatform('snes');
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('server-platform-header'))).toBeDisplayed();
    await expect($(testId('server-platform-counts'))).toHaveText('3 games on RomM · 0 installed');
    await expect($(testId('server-firmware-chip'))).toBeDisplayed();
    await expect($(testId('server-emulator-chip'))).toBeDisplayed();
  });

  it('routes the default-emulator chip to the system’s row in Settings › Systems', async () => {
    await $(testId('server-emulator-chip')).click();
    await $(testId('settings-page-systems')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Settings › Systems never opened from the chip',
    });
    // Opened on platform 1's row (SNES), its picker in view.
    await $(testId('sys-pick-snes')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(
      async () => ((await $(testId('sys-row-snes')).getAttribute('class')) ?? '').split(/\s+/).includes('open'),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the chip did not open the SNES row' },
    );
    await expect($(testId('sys-row-snes'))).toBeDisplayedInViewport();

    await openRommPlatform('snes');
  });

  it('filters the Server grid client-side from its search box', async () => {
    await openRommPlatform('snes');
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

  it('matches the Server search box against a game genre and its platform label', async () => {
    await openRommPlatform('snes');
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    // "Platformer" is only in Super Mario World's genres, never in its title.
    await $(testId('server-search')).setValue('platformer');
    await $(testId('game-card-102')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'Chrono Trigger survived a search for the genre "platformer"',
    });
    await expect($(testId('game-card-101'))).toExist();

    await $(testId('server-search')).setValue('');
    await $(testId('game-card-102')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clearing the search never restored the full platform list',
    });

    // The platform label matches too, case-insensitively. Only Super Mario
    // World's fixture carries the label, so the other card must drop out.
    await $(testId('server-search')).setValue('SUPER NINTENDO');
    await $(testId('game-card-102')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'a search for the platform label never filtered the grid',
    });
    await expect($(testId('game-card-101'))).toExist();

    await $(testId('server-search')).setValue('');
  });

  it('Ctrl+F focuses the active view search box', async () => {
    await openRommPlatform('snes');
    await browser.keys(['Control', 'f']);
    const focused = await browser.execute(
      () => document.activeElement?.getAttribute('data-testid') ?? '',
    );
    expect(focused).toBe('server-search');

    await $(testId('library-rail-all')).click();
    await browser.keys(['Control', 'f']);
    const libraryFocused = await browser.execute(
      () => document.activeElement?.getAttribute('data-testid') ?? '',
    );
    expect(libraryFocused).toBe('library-search');
  });

  it('Ctrl+1..3 switch between the three views, and there is no fourth', async () => {
    const views = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="shell-topbar"] nav [data-testid^="nav-"]')).map(
        (b) => (b as HTMLElement).dataset.testid,
      ),
    );
    expect(views).toEqual(['nav-library', 'nav-downloads', 'nav-settings']);
    // Focus off any text field: the accelerators stand aside for one.
    await browser.execute(() => (document.activeElement as HTMLElement | null)?.blur());
    const shown = async (view: string) => $(testId(`${view}-view`)).isDisplayed();
    await browser.keys(['Control', '3']);
    await browser.waitUntil(() => shown('settings'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Ctrl+3 never opened Settings' });
    await browser.keys(['Control', '2']);
    await browser.waitUntil(() => shown('downloads'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Ctrl+2 never opened Downloads' });
    await browser.keys(['Control', '4']);
    await browser.pause(300);
    expect(await shown('downloads')).toBe(true);
    await browser.keys(['Control', '1']);
    await browser.waitUntil(() => shown('library'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Ctrl+1 never opened Library' });
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

  it('shares one card size between a RomM platform and the Library grids', async () => {
    // The Server view's own card size went with the Server view
    // (romm-storefront-plan Step 3): a platform screen uses the Library's.
    await openRommPlatform('snes');
    await selectValue('server-size', 'large');
    await $(testId('library-rail-all')).click();
    await expect($(testId('library-size'))).toHaveValue('large');
    await selectValue('library-size', 'small');

    await openRommPlatform('snes');
    await expect($(testId('server-size'))).toHaveValue('small');
    await $(testId('library-rail-all')).click();
    await expect($(testId('library-size'))).toHaveValue('small');
  });
});
