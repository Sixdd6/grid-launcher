import { INSTALL_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  AMAZON,
  amazonCard,
  amazonTitles,
  closeWithEscape,
  coverTags,
  detailsLine,
  openAmazon,
  openAmazonDetails,
  startApp,
  testId,
} from '../helpers/amazon.js';
import { openDownloads, openStoresSettings, waitForDownloadLine } from '../helpers/gog.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `amazon-library` (GameStores README, "Library", "Install dialog",
 * "Details for a store game", "Settings › Stores"; docs/stores-phase3-plan.md
 * "E2E plan" and "Parity inventory"): the Amazon Games rail group first, its
 * counts, the Installed and Available views and their header (no Cloud saves
 * chip; Refresh only), an install through the dialog (no Version, Language
 * or DLC), update ("Update", no version), verify ("Verified", no count),
 * uninstall (no cloud sentence), Details with Overview and Media only, Game
 * Settings with no cloud switch, and the Settings card with no Cloud saves
 * row and the Disconnect confirm. Amazon is the first store with a tool but
 * no cloud saves and no DLC. Needs packages A and C (the mock Amazon
 * server, fake nile and seed): it goes green only after they are merged.
 */
describe('amazon-library', () => {
  before(async () => {
    await startApp();
  });

  it('shows the Amazon Games group first, with Installed and Available counts', async () => {
    await $(testId('library-rail-store-amazon-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-rail-count-store-amazon-installed'))).toHaveText(String(AMAZON.installed.length));
    await browser.waitUntil(
      async () => (await $(testId('library-rail-count-store-amazon-available')).getText()).trim() === String(AMAZON.available.length),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Available never counted the owned games' },
    );
    // Rail order is alphabetical by name: Amazon Games before Epic Games and GOG.
    const keys = await $$('[data-testid^="library-rail-store-"][data-testid$="-installed"]').map((el) => el.getAttribute('data-testid'));
    expect(keys[0]).toBe('library-rail-store-amazon-installed');
    // The store is no platform row (its installed rows' platform is "Amazon Games").
    const platforms = await $$('[data-testid^="library-rail-platform-"]').map((el) => el.getText());
    expect(platforms.some((t) => t.includes('Amazon Games'))).toBe(false);
  });

  it('Amazon Games › Installed lists Fogline with the account chip, no Cloud saves chip and no GOG-only actions', async () => {
    await openAmazon('installed');
    await expect($(testId('library-title'))).toHaveText('Amazon Games · Installed');
    await expect($(testId('library-count'))).toHaveText(/^1 game · .+ (GB|MB)$/);
    await expect($(testId('store-chip-account'))).toHaveText(`Signed in as ${AMAZON.account}`);
    await expect($(testId('store-chip-cloud'))).not.toExist();
    await expect($(testId('library-store-installer'))).not.toExist();
    await expect($(testId('library-store-backup-all'))).not.toExist();
    await expect($(testId('library-select'))).not.toExist();
    expect(await amazonTitles()).toEqual([...AMAZON.installed]);
    const { badge } = await amazonCard('Fogline');
    // No cloud button on an Amazon cover.
    await expect($(testId(`card-cloud-${badge}`))).not.toExist();
  });

  it('Amazon Games › Available lists the owned games, with Refresh', async () => {
    await openAmazon('available');
    await expect($(testId('library-title'))).toHaveText('Amazon Games · Available');
    await expect($(testId('library-count'))).toHaveText(/^2 games you own and haven’t installed · updated /);
    await expect($(testId('library-store-refresh'))).toHaveText('Refresh');
    await expect($(testId('library-store-installer'))).not.toExist();
    expect((await amazonTitles()).sort()).toEqual([...AMAZON.available].sort());
    for (const title of AMAZON.available) {
      const { badge } = await amazonCard(title);
      expect(await coverTags(badge)).not.toContain('NOT SUPPORTED');
    }
  });

  it('installs through the dialog: no Version, Language or DLC; a Downloads row and the installed card', async () => {
    await openAmazon('available');
    const { badge } = await amazonCard('Kettle Drum');
    await $(testId(`library-store-card-${badge}`)).moveTo();
    await $(testId(`card-primary-${badge}`)).click();
    await $(testId('store-install-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-install-sub'))).toHaveText(/^Amazon Games · .+ download$/);
    // nile installs the Windows build, in one language, with no DLC.
    await expect($('[data-testid^="store-install-version-"]')).not.toExist();
    await expect($(testId('store-install-language'))).not.toExist();
    await expect($('[data-testid^="store-install-dlc-"]')).not.toExist();
    await expect($(testId('store-install-compat'))).toExist();
    await expect($(testId('store-install-space'))).toHaveText(/ on disk · .+ free on /);
    await expect($(testId('store-install-download'))).toHaveText(/ to download$/);
    await $(testId('store-install-go')).click();
    await waitForToast('Kettle Drum added to download queue');
    await openDownloads();
    await waitForDownloadLine(/^(Downloading \d+% · |Waiting for a slot|Installing \d+%|Installed · )/);
    await waitForDownloadLine(/^Installed · /, INSTALL_TIMEOUT);
    await waitForToast('Kettle Drum installed', { timeout: INSTALL_TIMEOUT });
    await openAmazon('installed');
    expect(await amazonTitles()).toContain('Kettle Drum');
  });

  it('opens Details for an installed game: no Backup to drive, the runs and backup lines, Overview and Media only, no version', async () => {
    await openAmazon('installed');
    await openAmazonDetails('Fogline');
    await expect($(testId('store-details-chip-store'))).toHaveText('Amazon Games');
    // Amazon's version is an opaque id: no chip, no Version fact (user decision).
    await expect($(testId('store-details-chip-version'))).not.toExist();
    await expect($(testId('store-details'))).not.toHaveText(expect.stringContaining('VERSION'));
    await expect($(testId('store-details-play'))).toHaveText('Play');
    await expect($(testId('store-details-settings'))).toHaveText('Game Settings');
    await expect($(testId('store-details-verify'))).toHaveText('Verify and repair');
    await expect($(testId('store-details-backup'))).not.toExist();
    expect(await detailsLine('store-details-runs')).toMatch(/^Runs through .+ · Windows version$/);
    expect(await detailsLine('store-details-no-backup')).toBe(AMAZON.noBackup);
    await expect($(testId('store-details-cloud'))).not.toExist();
    await expect($(testId('store-details-tab-overview'))).toExist();
    await expect($(testId('store-details-tab-media'))).toExist();
    await expect($(testId('store-details-tab-cloud'))).not.toExist();
    await expect($(testId('store-details-dlc'))).not.toExist();
    await expect($(testId('details-tab-achievements'))).not.toExist();
    await closeWithEscape('store-details');
  });

  it('Game Settings: the executable is set by Amazon Games, the prefix is read-only, and there is no cloud switch', async () => {
    await openAmazon('installed');
    await openAmazonDetails('Fogline');
    await $(testId('store-details-settings')).click();
    await $(testId('store-game-settings')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-game-settings'))).toHaveText(expect.stringContaining('Set by Amazon Games.'));
    await expect($(testId('store-settings-cloud'))).not.toExist();
    await expect($(testId('store-game-settings'))).not.toHaveText(expect.stringContaining('cloud saves'));
    await closeWithEscape('store-game-settings');
    await closeWithEscape('store-details');
  });

  it('verifies an installed game: a Verifying row, then Verified (nile gives no count)', async () => {
    await openAmazon('installed');
    await openAmazonDetails('Fogline');
    await $(testId('store-details-verify')).click();
    await waitForToast(/^Verifying Fogline’s files…$/);
    // Verify goes away once pressed; Escape still closes Details (Phase 2 lesson).
    await closeWithEscape('store-details');
    await openDownloads();
    await waitForDownloadLine(/^Verified$/, INSTALL_TIMEOUT);
  });

  it('updates a game the seed marks out of date: UPDATE on its cover, “Update” with no version', async () => {
    await openAmazon('installed');
    // The fake nile lists Fogline in list-updates when the seed sets `.fake-update`.
    const { badge } = await amazonCard('Fogline');
    await browser.waitUntil(async () => (await coverTags(badge)).includes('UPDATE'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Fogline never carried UPDATE (the seed sets .fake-update)',
    });
    await openAmazonDetails('Fogline');
    const btn = $(testId('store-details-update'));
    await expect(btn).toHaveText('Update');
    await btn.click();
    await waitForToast('Update for Fogline added to download queue');
    await closeWithEscape('store-details');
    await openDownloads();
    await waitForDownloadLine(/^Updated$/, INSTALL_TIMEOUT);
  });

  it('uninstalls with two clicks, with no cloud sentence', async () => {
    await openAmazon('installed');
    await openAmazonDetails('Kettle Drum');
    await $(testId('store-details-uninstall')).click();
    await expect($(testId('store-details-uninstall-confirm'))).toHaveText('Confirm uninstall');
    await $(testId('store-details-uninstall-confirm')).click();
    await waitForToast(/^Kettle Drum uninstalled$/);
    await $(testId('store-details')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT }).catch(() => browser.keys('Escape'));
    await openAmazon('available');
    expect(await amazonTitles()).toContain('Kettle Drum');
  });

  it('Settings › Stores shows the Amazon Games card with no Cloud saves row and asks before Disconnect', async () => {
    await openStoresSettings();
    await expect($(testId('settings-stores-pane'))).toHaveText(
      expect.stringContaining(
        'GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder. Steam games install and start through the Steam client.',
      ),
    );
    await expect($(testId('store-card-pill-amazon'))).toHaveText(expect.stringContaining('Connected'));
    await expect($(testId('store-card-tool-amazon'))).toHaveText(/^nile .+ · installed$/);
    await expect($(testId('store-card-owned-amazon'))).toHaveText(/^3 · \d installed · updated /);
    await expect($(testId('store-cloud-switch-amazon'))).not.toExist();
    await expect($(testId('store-card-amazon'))).not.toHaveText(expect.stringContaining('Cloud saves'));
    await expect($(testId('settings-stores-pane'))).toHaveText(expect.stringContaining('nile · GPL-3.0 · github.com/imLinguin/nile'));
    await expect($(`${testId('store-card-amazon')} ${testId('store-installer-open-settings')}`)).not.toExist();
    await $(testId('store-disconnect-amazon')).click();
    await expect($(testId('store-disconnect-confirm-amazon'))).toHaveText(
      /Disconnect Amazon Games\? GRID deletes your Amazon Games login and nile from this computer\. The \d+ installed games? stay and still play; they can’t install or update until you connect again\./,
    );
    await $(testId('store-disconnect-cancel-amazon')).click();
    await expect($(testId('store-disconnect-confirm-amazon'))).not.toExist();
    await expect($(testId('store-card-pill-amazon'))).toHaveText(expect.stringContaining('Connected'));
  });
});
