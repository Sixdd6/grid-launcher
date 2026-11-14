import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectRomm, openGog, storeCards, testId } from '../helpers/gog.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `gog-cloud` (GameStores README, "Cloud saves tab", "Launch"): the
 * installed game the mock seeds with a cloud conflict (its key in
 * `E2E_GOG_CONFLICT_KEY`, e.g. `gog:1002`) shows SAVE CONFLICT on its cover;
 * Play opens "Which save should … use?" before the game starts, and a
 * choice launches it. Needs package D's mock GOG cloud storage and the store
 * branch of `launch_game_key`: it goes green only after integration.
 */
const KEY = process.env.E2E_GOG_CONFLICT_KEY ?? 'gog:1002';
const BADGE = KEY.replace(':', '-');

describe('gog-cloud', () => {
  before(async () => {
    await connectRomm();
    await openGog('installed');
  });

  it('tags the cover SAVE CONFLICT', async () => {
    await $(testId(`library-store-card-${BADGE}`)).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // The state is read in the background after sign-in, so the tag can come a moment later.
    let texts: string[] = [];
    await browser
      .waitUntil(
        async () => {
          const tags = await $$(`[data-testid="library-store-card-${BADGE}"] [data-testid="card-tag-${BADGE}"]`);
          // An element array's `map` is already async in WebdriverIO: await it, not Promise.all.
          texts = await tags.map((t) => t.getText());
          return texts.includes('SAVE CONFLICT');
        },
        { timeout: TRANSITION_TIMEOUT },
      )
      .catch(() => undefined);
    expect(texts).toContain('SAVE CONFLICT');
    expect((await storeCards()).length).toBeGreaterThan(0);
  });

  it('asks which save to use before launching, and launches after the choice', async () => {
    const card = $(testId(`library-store-card-${BADGE}`));
    await card.moveTo();
    await $(testId(`card-primary-${BADGE}`)).click();
    await $(testId('store-conflict-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-conflict-later'))).toHaveText('Don’t launch');
    // The cloud side's size comes from GOG's cloud listing (the mock's 2.0 MB save).
    await expect($(`${testId('store-conflict-dialog')} ${testId('store-conflict-sides')}`)).toHaveText(
      expect.stringMatching(/on another computer · 2\.0 MB/),
    );
    await $(testId('store-conflict-local')).click();
    await waitForToast(/^Kept this computer’s save for .+ and uploaded it$/);
    await waitForToast(/^Launching .+…$/);
  });

  it('shows the Cloud saves tab with its state and save locations', async () => {
    await $(testId(`library-store-card-${BADGE}`)).click();
    await $(testId('store-details-tab-cloud')).click();
    await expect($(testId('store-details-cloud-tab'))).toHaveText(expect.stringContaining('Save locations'));
    await browser.keys('Escape');
  });
});
