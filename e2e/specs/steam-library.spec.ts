import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openDownloads, openStoresSettings, waitForDownloadLine } from '../helpers/gog.js';
import {
  closeWithEscape,
  coverTags,
  detailsLine,
  dropManifest,
  keyOfBadge,
  openSteam,
  openSteamDetails,
  setMarker,
  startApp,
  STEAM,
  steamArgv,
  steamCard,
  steamCardAnswer,
  steamTitles,
  testId,
  waitArgv,
} from '../helpers/steam.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `steam-library` (GameStores README, "Steam"; docs/stores-phase4-plan.md
 * "E2E plan" and "Parity inventory", desktop). On package C's harness (the
 * mock Steam Web API, the fake `steam` client and the fake Steam root): the
 * Steam rail group last with its counts (tools never shown), the header
 * chips, Install in Steam (no dialog; the fake client's argv; the teal bar,
 * "Installing in Steam N%" and the managed box; a Downloads row with a
 * disabled Cancel; "Installed by Steam" and the card), the Details lines and
 * buttons (no Game Settings), Play through Steam (Playing, "Running in
 * Steam", the session ends when the fake clears RunningAppID), Verify and
 * Uninstall in Steam, a cancelled Steam install, a game installed in Steam
 * itself appearing within the background scan, and the Settings card's key
 * states (Remove key → Installed games only and the no-key panel). Steam is
 * the first store with no tool, no login window and installs its own client
 * runs. Green only once packages A and C are merged.
 */
/** The Downloads row whose title is `title`. */
async function downloadRow(title: string): Promise<WebdriverIO.Element | null> {
  for (const row of await $$('[data-testid^="download-row-"]')) {
    const t = await row.$('.title');
    if ((await t.isExisting()) && (await t.getText()).trim() === title) return row as unknown as WebdriverIO.Element;
  }
  return null;
}

describe('steam-library', () => {
  const INSTALL = 'Harbor of Bells';
  const CANCEL = 'Rime Assembly';

  before(async () => {
    await startApp();
  });

  it('shows the Steam group last, with 3 installed (never a tool or runtime) and 4 available', async () => {
    await $(testId('library-rail-store-steam-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await $(testId('library-rail-count-store-steam-installed')).getText()).trim() === '3', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Installed never counted the three games (tools must not count)',
    });
    await browser.waitUntil(async () => (await $(testId('library-rail-count-store-steam-available')).getText()).trim() === '4', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Available never counted the owned games',
    });
    // Alphabetical by name: Steam is the last store group.
    const keys = await $$('[data-testid^="library-rail-store-"][data-testid$="-installed"]').map((el) => el.getAttribute('data-testid'));
    expect(keys[keys.length - 1]).toBe('library-rail-store-steam-installed');
    // No "Steam" platform row.
    const platforms = await $$('[data-testid^="library-rail-platform-"]').map((el) => el.getText());
    expect(platforms.some((t) => t.includes('Steam'))).toBe(false);
  });

  it('Steam › Installed lists the three games, with the Steam account chip and no tool, cloud or backup actions', async () => {
    await openSteam('installed');
    await expect($(testId('library-title'))).toHaveText('Steam · Installed');
    await expect($(testId('library-count'))).toHaveText(/^3 games · .+ (GB|MB) · in your Steam libraries$/);
    await expect($(testId('store-chip-account'))).toHaveText(`Steam account: ${STEAM.persona}`);
    await expect($(testId('store-chip-tool'))).not.toExist();
    await expect($(testId('store-chip-cloud'))).not.toExist();
    await expect($(testId('library-store-refresh'))).not.toExist();
    await expect($(testId('library-store-installer'))).not.toExist();
    await expect($(testId('library-store-backup-all'))).not.toExist();
    await expect($(testId('library-select'))).not.toExist();
    expect(await steamTitles()).toEqual([...STEAM.installed]);
    for (const tool of STEAM.tools) expect(await steamTitles()).not.toContain(tool);
    const { badge } = await steamCard('Moonlit Ferry');
    await expect($(testId(`card-cloud-${badge}`))).not.toExist();
    expect(String(await $(testId(`card-primary-${badge}`)).getProperty('textContent')).trim()).toBe('Play');
  });

  it('Steam › Available lists the owned games with Install in Steam and Refresh, and no size', async () => {
    await openSteam('available');
    await expect($(testId('library-title'))).toHaveText('Steam · Available');
    await expect($(testId('library-count'))).toHaveText(/^4 games you own and haven’t installed · updated /);
    await expect($(testId('library-store-refresh'))).toHaveText('Refresh');
    expect([...(await steamTitles())].sort()).toEqual([...STEAM.available].sort());
    const { badge } = await steamCard(INSTALL);
    expect(String(await $(testId(`card-primary-${badge}`)).getProperty('textContent')).trim()).toBe('Install in Steam');
    for (const t of await coverTags(badge)) expect(['UPDATE', 'SAVE CONFLICT', 'NOT SUPPORTED']).not.toContain(t);
    // Overview gives no size for a game not installed (Steam's API has none).
    await openSteamDetails(INSTALL);
    await expect($(testId('store-details'))).not.toHaveText(expect.stringContaining('SIZE'));
    await expect($(testId('store-details'))).not.toHaveText(expect.stringContaining('GENRES'));
    await expect($(testId('store-details-install'))).toHaveText('Install in Steam');
    await expect($(testId('store-details-settings'))).not.toExist();
    await closeWithEscape('store-details');
  });

  it('Install in Steam opens Steam (no dialog), then follows the install to "Installed by Steam"', async () => {
    await openSteam('available');
    const { badge } = await steamCard(INSTALL);
    await $(testId(`library-store-card-${badge}`)).moveTo();
    await $(testId(`card-primary-${badge}`)).click();
    await expect($(testId('store-install-dialog'))).not.toExist();
    await waitForToast(`Steam opened to install ${INSTALL}. Choose the library folder there; GRID shows the progress.`);
    const appid = keyOfBadge(badge).split(':')[1];
    await waitArgv(new RegExp(`steam://install/${appid}\\b`));
    // While Steam installs: the percentage, the teal bar and a disabled Installing in Steam.
    await browser.waitUntil(async () => (await coverTags(badge)).some((t) => /^\d+%$/.test(t)), {
      timeout: TRANSITION_TIMEOUT * 2,
      timeoutMsg: 'the cover never showed the install’s percentage',
    });
    expect(await $(testId(`card-progress-${badge}`)).getAttribute('class')).toContain('managed');
    await $(testId(`library-store-card-${badge}`)).moveTo();
    const primary = $(testId(`card-primary-${badge}`));
    expect(String(await primary.getProperty('textContent')).trim()).toBe('Installing in Steam');
    await expect(primary).toBeDisabled();
    expect(await primary.getAttribute('title')).toBe('Managed by Steam');
    // Details: the managed box and a disabled Cancel.
    await $(testId(`library-store-card-${badge}`)).click();
    await $(testId('store-details')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-details-managed'))).toHaveText(/Installing in Steam · \d+%[\s\S]*Managed by Steam\. Pause or cancel it in Steam\./);
    await expect($(testId('store-details-cancel'))).toBeDisabled();
    await closeWithEscape('store-details');
    // Downloads: the row with no slot and a disabled Cancel, then Installed by Steam.
    await openDownloads();
    await waitForDownloadLine(/^Installing in Steam \d+% · Managed by Steam$/);
    const row = (await downloadRow(INSTALL))!;
    const cancel = await row.$('[data-testid^="download-action-cancel-"]');
    await expect(cancel).toBeDisabled();
    expect(await cancel.getAttribute('title')).toBe('Managed by Steam');
    await expect(row.$('[data-testid^="download-action-retry-"]')).not.toExist();
    await waitForDownloadLine(/^Installed by Steam$/, 20_000);
    await waitForToast(`${INSTALL} installed`, { timeout: 10_000 });
    await openSteam('installed');
    expect(await steamTitles()).toContain(INSTALL);
  });

  it('Details of a game installed in Steam itself: the lines, Verify and Uninstall in Steam, no Game Settings', async () => {
    await openSteam('installed');
    await openSteamDetails(STEAM.outside);
    await expect($(testId('store-details-chip-store'))).toHaveText('Steam');
    await expect($(testId('store-details-chip-version'))).not.toExist();
    await expect($(testId('store-details-chip-platform'))).not.toExist();
    await expect($(testId('store-details-play'))).toHaveText('Play');
    await expect($(testId('store-details-verify'))).toHaveText('Verify in Steam');
    await expect($(testId('store-details-uninstall'))).toHaveText('Uninstall in Steam…');
    await expect($(testId('store-details-settings'))).not.toExist();
    await expect($(testId('store-details-backup'))).not.toExist();
    await expect($(testId('store-details-update'))).not.toExist();
    expect(await detailsLine('store-details-runs')).toBe('Launches through Steam');
    expect(await detailsLine('store-details-outside')).toBe('Installed in Steam, outside GRID');
    expect(await detailsLine('store-details-no-backup')).toBe(STEAM.noBackup);
    await expect($(testId('store-details-cloud'))).not.toExist();
    await expect($(testId('store-details-tab-cloud'))).not.toExist();
    await expect($(testId('store-details'))).not.toHaveText(expect.stringContaining('FOLDER'));
    await closeWithEscape('store-details');
  });

  it('Play starts the game through Steam: Playing, Running in Steam, then the session ends on its own', async () => {
    await openSteam('installed');
    await openSteamDetails('Moonlit Ferry');
    await $(testId('store-details-play')).click();
    await waitForToast('Starting Moonlit Ferry through Steam…');
    await waitArgv(/^-silent steam:\/\/rungameid\/\d+$/);
    await $(testId('store-details-chip-playing')).waitForExist({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'Playing never showed' });
    const stop = $(testId('store-details-stop'));
    await expect(stop).toHaveText('Running in Steam');
    await stop.click();
    await waitForToast('Moonlit Ferry is running in Steam. Quit it from the game.');
    // The fake clears RunningAppID after E2E_STEAM_RUN_SECS (4 s): the session ends.
    await $(testId('store-details-chip-playing')).waitForExist({ reverse: true, timeout: 20_000, timeoutMsg: 'the Steam session never ended' });
    await expect($(testId('store-details-play'))).toHaveText('Play');
    expect(await detailsLine('store-details-last-played')).toMatch(/^Last played (just now|\d+ min ago)$/);
    await closeWithEscape('store-details');
  });

  it('Verify in Steam and Uninstall in Steam… open Steam; the game leaves Installed once Steam removes it', async () => {
    await openSteam('installed');
    const badge = await openSteamDetails('Old Brick Road');
    const appid = keyOfBadge(badge).split(':')[1];
    await $(testId('store-details-verify')).click();
    await waitForToast('Steam is verifying Old Brick Road’s files. Watch it in Steam.');
    await waitArgv(new RegExp(`steam://validate/${appid}\\b`));
    // No confirm of GRID's: Steam asks in its own window.
    await $(testId('store-details-uninstall')).click();
    await expect($(testId('store-details-uninstall-confirm'))).not.toExist();
    await waitForToast('Steam opened its uninstall window for Old Brick Road. It leaves the Library once Steam removes it.');
    await waitArgv(new RegExp(`steam://uninstall/${appid}\\b`));
    await $(testId('store-details')).waitForExist({ reverse: true, timeout: 10_000 }).catch(() => browser.keys('Escape'));
    await openSteam('installed');
    await browser.waitUntil(async () => !(await steamTitles()).includes('Old Brick Road'), {
      timeout: 20_000,
      timeoutMsg: 'Old Brick Road stayed in Installed after the fake removed its manifest',
    });
  });

  it('a Steam install that goes away ends Cancelled, with Dismiss only', async () => {
    setMarker('.fake-install-cancel');
    try {
      await openSteam('available');
      const { badge } = await steamCard(CANCEL);
      await $(testId(`library-store-card-${badge}`)).moveTo();
      await $(testId(`card-primary-${badge}`)).click();
      await waitForToast(`Steam opened to install ${CANCEL}. Choose the library folder there; GRID shows the progress.`);
      await openDownloads();
      await browser.waitUntil(
        async () => {
          const row = await downloadRow(CANCEL);
          return row !== null && /Cancelled/.test(await row.getText());
        },
        { timeout: 60_000, timeoutMsg: `${CANCEL}’s Steam row never ended Cancelled` },
      );
      const row = (await downloadRow(CANCEL))!;
      await expect(row.$('[data-testid^="download-action-retry-"]')).not.toExist();
      await expect(row.$('[data-testid^="download-action-dismiss-"]')).toExist();
    } finally {
      setMarker('.fake-install-cancel', false);
    }
  });

  it('a game installed in Steam itself shows in Installed within the background scan', async () => {
    dropManifest(9_900_001, 'Lantern Signal');
    await openSteam('installed');
    await steamCard('Lantern Signal', 30_000);
  });

  it('Settings › Stores: the Steam card, connected, with the key as dots only and no part of it', async () => {
    await openStoresSettings();
    await expect($(testId('store-card-pill-steam'))).toHaveText(expect.stringContaining('Connected'));
    await expect($(testId('store-card-account-steam'))).toHaveText(`SteamID64 ${STEAM.steamId} · ${STEAM.persona}`);
    await expect($(testId('steam-client-line'))).toHaveText(expect.stringContaining('Steam client found · '));
    // The local accounts by persona only, never the login name.
    await expect($(testId('steam-accounts'))).toHaveText(expect.stringContaining(STEAM.persona));
    await expect($(testId('steam-accounts'))).toHaveText(expect.stringContaining('couchguest'));
    await expect($(testId('settings-stores-pane'))).not.toHaveText(expect.stringContaining(STEAM.accountName));
    const saved = $(testId('steam-key-saved'));
    await expect(saved).toBeDisabled();
    expect(await saved.getValue()).toMatch(/^•+$/);
    expect(await $(testId('settings-stores-pane')).getHTML()).not.toContain(STEAM.key);
  });

  it('Remove leaves Steam connected with installed games only, and Available asks for the key', async () => {
    await $(testId('steam-key-remove')).click();
    await waitForToast('Web API key removed from the keyring. Steam’s Available list needs it.');
    await expect($(testId('store-card-pill-steam'))).toHaveText(expect.stringContaining('Installed games only'));
    await $(testId('steam-key-field')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await openSteam('available');
    await $(testId('store-state-nokey')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-state-nokey'))).toHaveText(expect.stringContaining('Steam needs your Web API key to list the games you haven’t installed'));
    await expect($(testId('library-rail-count-store-steam-available'))).toHaveText('!');
    await $(testId('store-state-nokey-open_settings')).click();
    await $(testId('settings-stores-pane')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  });

  it('Save with the key again: Steam connected, the field cleared, Available lists the games', async () => {
    await $(testId('steam-key-field')).setValue(STEAM.key);
    expect(await steamCardAnswer('steam-save')).toMatch(/^The key works · \d+ games owned$/);
    await waitForToast(/^Steam connected · \d+ games owned$/);
    await expect($(testId('store-card-pill-steam'))).toHaveText(expect.stringContaining('Connected'));
    await expect($(testId('steam-key-field'))).not.toExist();
    expect(await $(testId('settings-stores-pane')).getHTML()).not.toContain(STEAM.key);
    expect(steamArgv().join('\n')).not.toContain(STEAM.key);
  });
});
