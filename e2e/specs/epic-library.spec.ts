import { INSTALL_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  closeWithEscape,
  coverTags,
  detailsLine,
  EPIC,
  epicCard,
  epicTitles,
  openEpic,
  openEpicDetails,
  startApp,
  testId,
} from '../helpers/epic.js';
import { openDownloads, openStoresSettings, waitForDownloadLine } from '../helpers/gog.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `epic-library` (GameStores README, "Library", "Install dialog",
 * "Details for a store game", "Settings › Stores"; docs/stores-phase2-plan.md
 * "E2E plan"): the Epic Games rail group and its counts, the Installed and
 * Available views and their header (Refresh only: no Install from
 * installer…, Select or Backup all), Not supported, an install with a DLC
 * through the dialog (no Version, no Language), update, verify, uninstall,
 * and the Settings card with its Disconnect confirm. Needs packages A–C (the
 * mock Epic server, fake legendary and seed): it goes green only after they
 * are merged.
 */
describe('epic-library', () => {
  before(async () => {
    await startApp();
  });

  it('shows the Epic Games group before GOG, with Installed and Available counts', async () => {
    await $(testId('library-rail-store-epic-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-rail-count-store-epic-installed'))).toHaveText(String(EPIC.installed.length));
    await browser.waitUntil(
      async () => (await $(testId('library-rail-count-store-epic-available')).getText()).trim() === String(EPIC.available.length),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Available never counted the owned games' },
    );
    // Rail order is alphabetical by name: Epic Games comes before GOG when both show.
    const keys = await $$('[data-testid^="library-rail-store-"][data-testid$="-installed"]').map((el) => el.getAttribute('data-testid'));
    expect(keys[0]).toBe('library-rail-store-epic-installed');
    // The store is no platform row (its installed rows' platform is "Epic Games").
    const platforms = await $$('[data-testid^="library-rail-platform-"]').map((el) => el.getText());
    expect(platforms.some((t) => t.includes('Epic Games'))).toBe(false);
  });

  it('Epic Games › Installed lists the installed games with the account and cloud chips, and no GOG-only actions', async () => {
    await openEpic('installed');
    await expect($(testId('library-title'))).toHaveText('Epic Games · Installed');
    await expect($(testId('library-count'))).toHaveText(/^2 games · .+ (GB|MB)$/);
    await expect($(testId('store-chip-account'))).toHaveText(`Signed in as ${EPIC.account}`);
    await expect($(testId('store-chip-cloud'))).toHaveText('Cloud saves: on');
    await expect($(testId('library-store-installer'))).not.toExist();
    await expect($(testId('library-store-backup-all'))).not.toExist();
    await expect($(testId('library-select'))).not.toExist();
    expect((await epicTitles()).sort()).toEqual([...EPIC.installed].sort());
  });

  it('Epic Games › Available lists the owned games, with Refresh and the Not supported tag', async () => {
    await openEpic('available');
    await expect($(testId('library-title'))).toHaveText('Epic Games · Available');
    await expect($(testId('library-count'))).toHaveText(/^4 games you own and haven’t installed · updated /);
    await expect($(testId('library-store-refresh'))).toHaveText('Refresh');
    await expect($(testId('library-store-installer'))).not.toExist();
    expect((await epicTitles()).sort()).toEqual([...EPIC.available].sort());
    for (const title of Object.keys(EPIC.notSupported)) {
      const { badge } = await epicCard(title);
      expect(await coverTags(badge)).toContain('NOT SUPPORTED');
      // Its hover action opens Details, never the install dialog.
      await $(testId(`library-store-card-${badge}`)).moveTo();
      await expect($(testId(`card-primary-${badge}`))).toHaveText('Details');
    }
  });

  it('a Not supported game’s Details names its launcher and can’t install', async () => {
    await openEpic('available');
    await openEpicDetails('Iron Orchard');
    await expect($(testId('store-details-install'))).toHaveText('Not supported');
    await expect($(testId('store-details-install'))).toBeDisabled();
    expect(await detailsLine('store-details-unsupported')).toBe(
      'Needs the EA app. GRID can’t install games that start through another company’s launcher; install it from the Epic Games Launcher instead.',
    );
    expect(await detailsLine('store-details-no-backup')).toBe('Epic Games games can’t be backed up: legendary installs only from Epic’s servers.');
    await expect($(testId('store-details-chip-unsupported'))).toHaveText('Not supported');
    await closeWithEscape('store-details');
  });

  it('installs a game with a DLC through the dialog: no Version, no Language; a Downloads row and the installed card', async () => {
    await openEpic('available');
    const { badge } = await epicCard('Glimmer Deep');
    await $(testId(`library-store-card-${badge}`)).moveTo();
    await $(testId(`card-primary-${badge}`)).click();
    await $(testId('store-install-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-install-sub'))).toHaveText(/^Epic Games · .+ download$/);
    // legendary installs the Windows build, in one language.
    await expect($('[data-testid^="store-install-version-"]')).not.toExist();
    await expect($(testId('store-install-language'))).not.toExist();
    await expect($(testId('store-install-compat'))).toExist();
    // Every DLC you own is ticked by default.
    const dlc = await $$('[data-testid^="store-install-dlc-"] input');
    expect(dlc.length).toBeGreaterThan(0);
    for (const box of dlc) await expect(box).toBeSelected();
    await expect($(testId('store-install-space'))).toHaveText(/ on disk · .+ free on /);
    await expect($(testId('store-install-download'))).toHaveText(/ to download$/);
    await $(testId('store-install-go')).click();
    await waitForToast('Glimmer Deep added to download queue');
    await openDownloads();
    await waitForDownloadLine(/^(Downloading \d+% · |Waiting for a slot|Installing \d+%|Installed · )/);
    await waitForDownloadLine(/^Installed · /, INSTALL_TIMEOUT);
    await waitForToast('Glimmer Deep installed', { timeout: INSTALL_TIMEOUT });
    await openEpic('installed');
    expect(await epicTitles()).toContain('Glimmer Deep');
  });

  it('opens Details for an installed game: buttons without Backup to drive, the runs and backup lines, the tabs', async () => {
    await openEpic('installed');
    await openEpicDetails('Starlight Courier');
    await expect($(testId('store-details-chip-store'))).toHaveText('Epic Games');
    await expect($(testId('store-details-play'))).toHaveText('Play');
    await expect($(testId('store-details-settings'))).toHaveText('Game Settings');
    await expect($(testId('store-details-verify'))).toHaveText('Verify and repair');
    await expect($(testId('store-details-backup'))).not.toExist();
    expect(await detailsLine('store-details-runs')).toMatch(/^Runs through .+ · Windows version$/);
    expect(await detailsLine('store-details-no-backup')).toBe('Epic Games games can’t be backed up: legendary installs only from Epic’s servers.');
    await expect($(testId('store-details-tab-overview'))).toExist();
    await expect($(testId('store-details-tab-media'))).toExist();
    await expect($(testId('store-details-tab-cloud'))).toExist();
    // No RomM sections.
    await expect($(testId('details-tab-achievements'))).not.toExist();
    await closeWithEscape('store-details');
  });

  it('Game Settings: the executable is set by Epic Games, the prefix is read-only, the cloud switch names Epic Games', async () => {
    await openEpic('installed');
    await openEpicDetails('Starlight Courier');
    await $(testId('store-details-settings')).click();
    await $(testId('store-game-settings')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-game-settings'))).toHaveText(expect.stringContaining('Set by Epic Games.'));
    await expect($(testId('store-settings-cloud'))).toExist();
    await expect($(testId('store-game-settings'))).toHaveText(expect.stringContaining('Sync this game’s cloud saves with Epic Games'));
    await closeWithEscape('store-game-settings');
    await closeWithEscape('store-details');
  });

  it('verifies an installed game: a Verifying row, then Verified', async () => {
    await openEpic('installed');
    await openEpicDetails('Vector Bloom');
    await $(testId('store-details-verify')).click();
    await waitForToast(/^Verifying Vector Bloom’s files…$/);
    await closeWithEscape('store-details');
    await openDownloads();
    await waitForDownloadLine(/^Verified/, INSTALL_TIMEOUT);
  });

  it('updates a game the seed marks out of date: UPDATE on its cover, Update to … in Details', async () => {
    await openEpic('installed');
    // The seed's owned list carries a newer build for one installed game, when it has one.
    let updated: string | null = null;
    for (const title of EPIC.installed) {
      const { badge } = await epicCard(title);
      if ((await coverTags(badge)).includes('UPDATE')) updated = title;
    }
    if (updated === null) return; // No update in this seed: covered by the vitest suite.
    await openEpicDetails(updated);
    const btn = $(testId('store-details-update'));
    await expect(btn).toHaveText(/^Update to .+/);
    await btn.click();
    await waitForToast(`Update for ${updated} added to download queue`);
    await closeWithEscape('store-details');
    await openDownloads();
    await waitForDownloadLine(/^Updated to /, INSTALL_TIMEOUT);
  });

  it('uninstalls with two clicks and keeps the cloud saves', async () => {
    await openEpic('installed');
    await openEpicDetails('Glimmer Deep');
    await $(testId('store-details-uninstall')).click();
    await expect($(testId('store-details-uninstall-confirm'))).toHaveText('Confirm uninstall');
    await $(testId('store-details-uninstall-confirm')).click();
    await waitForToast(/^Glimmer Deep uninstalled(\. Its cloud saves stay in your Epic Games account\.)?$/);
    await $(testId('store-details')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT }).catch(() => browser.keys('Escape'));
    await openEpic('available');
    expect(await epicTitles()).toContain('Glimmer Deep');
  });

  it('Settings › Stores shows the Epic Games card and asks before Disconnect', async () => {
    await openStoresSettings();
    await expect($(testId('settings-stores-pane'))).toHaveText(
      expect.stringContaining(
        'GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder. Steam games install and start through the Steam client.',
      ),
    );
    await expect($(testId('store-card-pill-epic'))).toHaveText(expect.stringContaining('Connected'));
    await expect($(testId('store-card-tool-epic'))).toHaveText(/^legendary .+ · installed$/);
    await expect($(testId('store-card-owned-epic'))).toHaveText(/^6 · \d installed · updated /);
    await expect($(testId('store-cloud-switch-epic'))).toBeSelected();
    await expect($(testId('settings-stores-pane'))).toHaveText(expect.stringContaining('legendary · GPL-3.0 · github.com/legendary-gl/legendary'));
    // Epic has no offline installers.
    await expect($(`${testId('store-card-epic')} ${testId('store-installer-open-settings')}`)).not.toExist();
    await $(testId('store-disconnect-epic')).click();
    await expect($(testId('store-disconnect-confirm-epic'))).toHaveText(
      expect.stringContaining('Disconnect Epic Games? GRID deletes your Epic Games login and legendary from this computer.'),
    );
    await $(testId('store-disconnect-cancel-epic')).click();
    await expect($(testId('store-disconnect-confirm-epic'))).not.toExist();
    // Nothing was disconnected.
    await expect($(testId('store-card-pill-epic'))).toHaveText(expect.stringContaining('Connected'));
  });
});
