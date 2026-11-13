import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectRomm, openGog, storeCards, testId } from '../helpers/gog.js';

/**
 * Stage `gog-backups` (GameStores README, "GOG games on backup drives"):
 * Select in GOG › Installed, the select bar, the GOG backup dialog with its
 * Installer files / Installed copy rows, and Backup all N… in the header.
 * Needs a set-up drive (the backups stages' loop-device drive) and package
 * C's `plan_store_backup`: it goes green only after integration.
 */
describe('gog-backups', () => {
  before(async () => {
    await connectRomm();
    await openGog('installed');
  });

  it('picks GOG games in Select mode and opens the backup dialog', async () => {
    await $(testId('library-select')).click();
    const cards = await storeCards();
    expect(cards.length).toBeGreaterThan(0);
    await cards[0].click();
    await expect($(testId('library-selection-count'))).toHaveText('1 selected');
    await $(testId('library-selection-backup')).click();
    await $(testId('store-backup-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-backup-title'))).toHaveText('Backup 1 game');
    await browser.waitUntil(async () => (await $(testId('store-backup-rows')).isExisting()) || (await $(testId('store-backup-no-drives')).isExisting()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the dialog never planned the backup',
    });
    if (await $(testId('store-backup-rows')).isExisting()) {
      await expect($(testId('store-backup-rows'))).toHaveText(/Installer files|Installed copy/);
    }
    await browser.keys('Escape');
    await browser.keys('Escape');
  });

  it('offers Backup all N… in the GOG › Installed header', async () => {
    await expect($(testId('library-store-backup-all'))).toHaveText(/^Backup all \d+…$/);
  });
});
