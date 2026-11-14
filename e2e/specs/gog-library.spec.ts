import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectRomm, openDownloads, openGog, openStoresSettings, storeCards, testId, waitForDownloadLine } from '../helpers/gog.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `gog-library` (GameStores README, "Library", "Install dialog",
 * "Details for a store game", "Settings › Stores"): the GOG rail group, the
 * Installed and Available views, a store Details, an install through the
 * dialog to a Downloads row and its card, and the Settings card with its
 * Disconnect confirm. Needs package D's mock GOG server and fake gogdl: it
 * goes green only once packages A–D are merged.
 */
describe('gog-library', () => {
  before(async () => {
    await connectRomm();
  });

  it('shows the GOG group under Backup drives with Installed and Available counts', async () => {
    await $(testId('library-rail-store-gog-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-rail-count-store-gog-installed'))).toHaveText(/^\d+$/);
    await expect($(testId('library-rail-store-gog-available'))).toExist();
  });

  it('GOG › Installed lists installed games with the header chips and actions', async () => {
    await openGog('installed');
    await expect($(testId('library-count'))).toHaveText(/^\d+ games? · .+ (GB|MB)$/);
    await expect($(testId('store-chip-account'))).toHaveText(expect.stringMatching(/^Signed in as /));
    await expect($(testId('library-store-installer'))).toHaveText('Install from installer…');
    expect((await storeCards()).length).toBeGreaterThan(0);
  });

  it('GOG › Available lists owned games and words the count line', async () => {
    await openGog('available');
    await expect($(testId('library-count'))).toHaveText(/you own and haven’t installed · updated /);
    await expect($(testId('library-store-refresh'))).toHaveText('Refresh');
  });

  it('installs an available game through the dialog: a Downloads row and the queued card', async () => {
    await openGog('available');
    const card = (await storeCards())[0];
    await card.moveTo();
    const title = (await card.$('.label').getText()).trim();
    await card.$(testId(`card-primary-${(await card.getAttribute('data-testid')).replace('library-store-card-', '')}`)).click();
    await $(testId('store-install-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-install-download'))).toHaveText(/ to download$/);
    await $(testId('store-install-go')).click();
    await waitForToast(`${title} added to download queue`);
    await openDownloads();
    await waitForDownloadLine(/^(Downloading \d+% · |Waiting for a slot|Installing \d+%)/);
  });

  it('opens Details for an installed game: no RomM sections, store chips and tabs', async () => {
    await openGog('installed');
    const card = (await storeCards())[0];
    await card.click();
    await $(testId('store-details')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-details-chip-store'))).toHaveText('GOG');
    await expect($(testId('store-details-tab-overview'))).toExist();
    await expect($(testId('store-details-play'))).toHaveText('Play');
    await expect($(testId('store-details-uninstall'))).toHaveText('Uninstall');
    // No RomM sections.
    await expect($(testId('details-tab-achievements'))).not.toExist();
    await expect($(testId('details-unlinked'))).not.toExist();
    // Uninstall takes two clicks.
    await $(testId('store-details-uninstall')).click();
    await expect($(testId('store-details-uninstall-confirm'))).toHaveText('Confirm uninstall');
    await browser.keys('Escape');
    await $(testId('store-details')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
  });

  it('Game Settings lists the game’s executables, without its uninstaller or Wine prefix', async () => {
    await openGog('installed');
    await $(testId('library-store-card-gog-1580232252')).click();
    await $(testId('store-details')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('store-details-settings')).click();
    const exe = $(testId('store-settings-exe'));
    await browser.waitUntil(async () => (await exe.$$('option').length) > 0, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Executable list never filled',
    });
    const options = await exe.$$('option').map((o) => o.getText());
    expect(options).toEqual(['game.exe', 'Launcher.exe']);
    await expect(exe).toHaveValue(expect.stringMatching(/game\.exe$/));
    await browser.keys('Escape');
    await $(testId('store-game-settings')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    await browser.keys('Escape');
    await $(testId('store-details')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
  });

  it('Settings › Stores shows the GOG card and asks before Disconnect', async () => {
    await openStoresSettings();
    await expect($(testId('store-card-pill-gog'))).toHaveText(expect.stringContaining('Connected'));
    await expect($(testId('store-card-tool-gog'))).toHaveText(/gogdl .* · installed/);
    await $(testId('store-disconnect-gog')).click();
    await expect($(testId('store-disconnect-confirm-gog'))).toHaveText(expect.stringContaining('Disconnect GOG?'));
    // This card's own Cancel (another view's Cancel may come first in the page).
    await $(testId('store-disconnect-cancel-gog')).click();
    await expect($(testId('store-disconnect-confirm-gog'))).not.toExist();
  });
});
