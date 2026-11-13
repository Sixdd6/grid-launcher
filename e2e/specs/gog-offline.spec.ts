import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectRomm, openDownloads, openGog, testId, waitForDownloadLine } from '../helpers/gog.js';

/**
 * Stage `gog-offline` (GameStores README, "Offline GOG installer"): Install
 * from installer… in the GOG header, the path typed into the dialog (the
 * E2E build has no file chooser), "Reading the installer…", the recognised
 * card and a refusal with Choose another file…, then the Installer row in
 * Downloads. The installer files come from package D's fixtures
 * (`E2E_GOG_INSTALLER_SH`, `E2E_GOG_INSTALLER_PKG`): it goes green only once
 * packages B and D are merged.
 */
const SH = process.env.E2E_GOG_INSTALLER_SH ?? '';
const PKG = process.env.E2E_GOG_INSTALLER_PKG ?? '';

async function openInstaller(path: string): Promise<void> {
  await openGog('installed');
  await $(testId('library-store-installer')).click();
  await $(testId('store-installer-path')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  await $(testId('store-installer-path')).setValue(path);
  await $(testId('store-installer-open')).click();
}

describe('gog-offline', () => {
  before(async () => {
    await connectRomm();
  });

  it('refuses a macOS installer and offers another file', async () => {
    if (PKG === '') return;
    await openInstaller(PKG);
    await $(testId('store-installer-refused')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-installer-refused'))).toHaveText(expect.stringContaining('This is a macOS installer'));
    await $(testId('store-installer-another')).click();
    await expect($(testId('store-installer-path'))).toBeDisplayed();
    await browser.keys('Escape');
  });

  it('recognises a Linux installer, installs it and shows the Installer row', async () => {
    if (SH === '') return;
    await openInstaller(SH);
    await $(testId('store-installer-found')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-installer-found'))).toHaveText(expect.stringContaining('Linux version'));
    await expect($(testId('store-installer-how'))).toHaveText('GRID unpacks the game files from it. The installer script isn’t run.');
    await $(testId('store-installer-install')).click();
    await openDownloads();
    await waitForDownloadLine(/^(Unpacking the game files \d+%|Installed from its installer|Waiting for a slot)$/);
  });
});
