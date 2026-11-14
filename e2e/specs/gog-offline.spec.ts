import path from 'node:path';
import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectRomm, openDownloads, openGog, testId, waitForDownloadLine } from '../helpers/gog.js';

/**
 * Stage `gog-offline` (GameStores README, "Offline GOG installer"): Install
 * from installer… in the GOG header, the path typed into the dialog (the
 * E2E build has no file chooser), "Reading the installer…", the recognised
 * card and a refusal with Choose another file…, then the Installer row in
 * Downloads. The installer files come from package D's fixtures
 * (`E2E_GOG_INSTALLER_SH`, `E2E_GOG_INSTALLER_PKG`): it goes green only once
 * packages B and D are merged. The two fake Windows installers
 * (seed/gog-seed.mjs, run through the `wine` stub) cover the attempts: Bramble's
 * first silent run fails and the milder one installs; Foxglove's silent runs
 * both fail, so the desktop opens its setup window, which installs.
 */
const SH = process.env.E2E_GOG_INSTALLER_SH ?? '';
const PKG = process.env.E2E_GOG_INSTALLER_PKG ?? '';
const DATA = process.env.E2E_DATA_DIR ?? '';
const exe = (name: string) => (DATA === '' ? '' : path.join(DATA, 'installers', name));
const BRAMBLE_EXE = exe('setup_bramble_1.0_(55002).exe');
const FOXGLOVE_EXE = exe('setup_foxglove_1.0_(55001).exe');

/** The Downloads line of the row titled `title` ('' when it has none). */
async function rowLine(title: string): Promise<string> {
  return (await browser.execute((t: string) => {
    const row = Array.from(document.querySelectorAll('[data-testid^="download-row-"]')).find((r) => (r.textContent ?? '').includes(t));
    return (row?.querySelector('[data-testid^="download-detail-"]')?.textContent ?? '').trim();
  }, title)) as string;
}

async function waitRowLine(title: string, re: RegExp, timeout = 60_000): Promise<void> {
  await browser.waitUntil(async () => re.test(await rowLine(title)), {
    timeout,
    interval: 250,
    timeoutMsg: `${title}'s Downloads line never matched ${re} (it reads "${await rowLine(title)}")`,
  });
}

async function installWindows(file: string): Promise<void> {
  await openInstaller(file);
  await $(testId('store-installer-found')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  await expect($(testId('store-installer-found'))).toHaveText(expect.stringContaining('Windows version'));
  await expect($(testId('store-installer-how'))).toHaveText(expect.stringContaining('GRID tries once more with gentler settings, then opens its own setup window'));
  await $(testId('store-installer-install')).click();
  await openDownloads();
}

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
    // What it needs, and the free space on the volume it installs to.
    await expect($(testId('store-installer-space'))).toHaveText(/^Needs (\d+(\.\d)? GB|\d+ MB|<1 MB) · \d+(\.\d)? GB free$/);
    await $(testId('store-installer-install')).click();
    await openDownloads();
    await waitForDownloadLine(/^(Unpacking the game files \d+%|Installed from its installer|Waiting for a slot)$/);
  });

  it('a Windows installer whose silent run fails installs on the milder silent run', async () => {
    if (DATA === '') return;
    await installWindows(BRAMBLE_EXE);
    await waitRowLine('Bramble', /^Installed from its installer$/);
  });

  it('a Windows installer whose silent runs both fail opens its setup window on the desktop', async () => {
    if (DATA === '') return;
    await installWindows(FOXGLOVE_EXE);
    await waitRowLine('Foxglove', /^Installed from its installer$/);
  });
});
