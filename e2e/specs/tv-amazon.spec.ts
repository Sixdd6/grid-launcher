// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, INSTALL_TIMEOUT, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, key, waitFocused, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, confirmDialog, dlLines, focusOn, pageRow, tap, tapK, waitNotice } from '../helpers/tvBackups.js';
import { loginWindowOpen, MAIN_WINDOW } from '../helpers/gogLogin.js';
import { AMAZON } from '../helpers/amazon.js';

/**
 * Stage `tv-amazon` (docs/stores-phase3-plan.md package D, E2E plan and
 * Parity inventory, TV; TVStores README § Library, § The game page,
 * § Settings › Stores, § Signing in). On package C's Amazon harness: the mock
 * Amazon server, the fake nile, and `seed/amazon-seed.mjs` (signed in as
 * sam.plays; Fogline installed; Kettle Drum and Lowsun owned). Enters TV
 * mode, opens Library › Amazon Games, installs a game through the install
 * page (no Version, Language or DLC rows), follows it in Downloads, opens an
 * installed game (LB / RB reach Overview and Media only, no Cloud saves) and
 * its Game settings (no cloud row), then Settings › Stores › Amazon Games:
 * its rows (no Cloud saves), the Disconnect confirm and the sign-in over TV
 * mode with the login window beside it (WebDriver goes back to the main
 * window). Green only once packages A and C are merged.
 */

/** The cover's `data-k` in the open grid for the game titled `title`. */
async function coverK(title: string): Promise<string> {
  let k: string | null = null;
  await browser.waitUntil(
    async () => {
      k = (await browser.execute(
        (t: string) =>
          Array.from(document.querySelectorAll('#lib-grid [data-cover]'))
            .find((c) => c.querySelector(`img[alt="${t}"]`) !== null || (c.textContent ?? '').includes(t))
            ?.getAttribute('data-k') ?? null,
        title,
      )) as string | null;
      return k !== null;
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${title} never showed in the grid` },
  );
  return k!;
}

/** The game page's button labels, in order. */
async function buttons(): Promise<string[]> {
  return (await $$('[data-testid="tv-store-details"] .btns .btn').map((b) => b.getText())).map((b) => b.trim());
}

describe('tv-amazon: Amazon Games in TV mode', () => {
  const INSTALL = 'Lowsun';
  const INSTALLED = 'Fogline';

  before(async () => {
    // The Amazon seed may start with or without a RomM server: connect when asked.
    await browser.waitUntil(async () => (await $(testId('connect-server-url')).isExisting()) || (await $(testId('tv-mode-button')).isExisting()), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'neither the connect form nor the shell appeared',
    });
    if (await $(testId('connect-server-url')).isExisting()) {
      await $(testId('connect-server-url')).setValue(mockUrl());
      await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
      await $(testId('connect-submit')).click();
    }
    await $(testId('tv-mode-button')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the shell never rendered' });
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await connectPad(XBOX_PAD);
  });

  after(async () => {
    expect(await pageErrors()).toEqual([]);
  });

  it('the rail has an Amazon Games folder first, with Installed and Available', async () => {
    // The tabs wrap (LB/RB), so go to the tab itself.
    await tap(testId('tv-tab-library'));
    await $(testId('tv-lib-fold-store-amazon')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no Amazon Games folder in the Library rail' });
    expect(await $(testId('tv-lib-fold-store-amazon')).getAttribute('aria-expanded')).toBe('true');
    expect(await text('tv-lib-fold-store-amazon')).toContain('Amazon Games');
    // Connected with nile installed: no `!` or `off` flag.
    expect(await $(testId('tv-lib-store-flag-amazon')).isExisting()).toBe(false);
    await $(testId('tv-lib-rail-store-amazon-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('tv-lib-rail-store-amazon-available')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Alphabetical: Amazon Games is the first store folder.
    const folds = (await $$('[data-testid^="tv-lib-fold-store-"]').map((f) => f.getAttribute('data-testid'))) as string[];
    expect(folds[0]).toBe('tv-lib-fold-store-amazon');
  });

  it('Available has the two-row header with Refresh, no Cloud saves chip, no installer button or Select', async () => {
    await tap(testId('tv-lib-rail-store-amazon-available'));
    await waitText('tv-lib-name', 'Amazon Games · Available');
    await waitText('tv-lib-count', /^\d+ games? you own and haven’t installed · updated /);
    expect(await text('tv-store-chip-account')).toBe(`Signed in as ${AMAZON.account}`);
    expect(await $(testId('tv-store-chip-cloud')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-installer')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-backup-all')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-refresh')).isExisting()).toBe(true);
    expect(await hints()).not.toEqual(expect.arrayContaining(['RT Select']));
  });

  it('Install… opens the install page with no Version, Language or DLC rows; Start queues it', async () => {
    const k = await coverK(INSTALL);
    await focusOn(byK(k));
    expect(await hints()).toEqual(expect.arrayContaining(['X Install…']));
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-det-title', INSTALL);
    expect(await text('tv-det-header')).toMatch(/^Amazon Games/);
    expect(await text('tv-store-line-runs')).toMatch(/^Windows only · runs through .+$/);
    expect(await text('tv-store-line-nobk')).toBe(AMAZON.noBackup);
    expect(await $(testId('tv-store-tab-Cloud saves')).isExisting()).toBe(false);
    // The page opens on its first enabled item: Install….
    await waitFocused('sd-install');
    await key('Enter');
    await $(testId('tv-store-install')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the install page never opened' });
    await waitText('tv-rows-title', `Install ${INSTALL}`);
    await waitText('tv-rows-sub', /^Amazon Games · .+ download$/);
    await waitText('tv-rows-summary', /^.+ to download · .+ free on .+$/);
    // Windows only, one language, no DLC.
    expect(await $(testId('tv-pr-ver-linux')).isExisting()).toBe(false);
    expect(await $(testId('tv-pr-ver-windows')).isExisting()).toBe(false);
    expect(await $(testId('tv-pr-lang')).isExisting()).toBe(false);
    expect(await $('[data-testid^="tv-pr-dlc-"]').isExisting()).toBe(false);
    expect(await text('tv-store-install')).not.toContain('DLC you own');
    expect(await pageRow('compat')).toMatchObject({ label: 'Compatibility tool', sub: 'The default from Settings › Systems › Windows' });
    expect(await pageRow('folder')).toMatchObject({ label: 'Install folder', sub: 'The default for Amazon Games games is in Settings › Stores' });
    expect(await hints()).toEqual(expect.arrayContaining(['MENU Install']));
    await key('m');
    await waitGone('tv-store-install');
    await waitNotice(`${INSTALL} added to download queue`, 10_000);
  });

  it('Downloads shows the store row with the Amazon Games badge until it is installed', async () => {
    // The game page is still open under the install page: go to the tab itself.
    await tap(testId('tv-tab-downloads'));
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(INSTALL)), { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${INSTALL} has no Downloads row` });
    const mine = (await dlLines()).find((r) => r.title.startsWith(INSTALL))!;
    expect(mine.kind).toContain('Amazon Games');
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(INSTALL) && /^Installed/.test(r.line)), {
      timeout: INSTALL_TIMEOUT * 3,
      timeoutMsg: `${INSTALL} never finished installing`,
    });
    await waitNotice(new RegExp(`^${INSTALL} installed`), 10_000);
  });

  it('Installed: an installed game has Play, no Backup to drive…, no version and no cloud line', async () => {
    await tap(testId('tv-tab-library'));
    await tap(testId('tv-lib-rail-store-amazon-installed'));
    await waitText('tv-lib-name', 'Amazon Games · Installed');
    expect(await $(testId('tv-store-backup-all')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-installer')).isExisting()).toBe(false);
    expect(await hints()).not.toEqual(expect.arrayContaining(['RT Select']));
    const k = await coverK(INSTALLED);
    await focusOn(byK(k));
    await browser.waitUntil(async () => (await hints()).includes('X Play'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X Play never showed' });
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-det-title', INSTALLED);
    const b = await buttons();
    expect(b).toEqual(expect.arrayContaining(['Play', 'Game settings', 'Verify and repair', 'Uninstall']));
    expect(b).not.toContain('Backup to drive…');
    // An update, when the seed lists one, reads "Update" with no version.
    expect(b.filter((x) => x.startsWith('Update'))).toEqual(b.includes('Update') ? ['Update'] : []);
    expect(await text('tv-store-line-runs')).toMatch(/^Runs through .+ · Windows version$/);
    expect(await text('tv-store-line-nobk')).toBe(AMAZON.noBackup);
    expect(await $(testId('tv-store-line-cloud')).isExisting()).toBe(false);
    // Amazon's version is an opaque id: no "v…" chip.
    expect(await text('tv-det-header')).not.toMatch(/\bv\d/);
  });

  it('LB / RB move between Overview and Media only, and wrap', async () => {
    expect(await $(testId('tv-store-tab-Cloud saves')).isExisting()).toBe(false);
    // Overview → Media → (wraps) Overview.
    await key('e');
    await key('e');
    expect(await $(testId('tv-store-cloud')).isExisting()).toBe(false);
    expect(await text('tv-det-body')).not.toContain('VERSION');
  });

  it('Game settings has the launch file set by Amazon Games and no cloud row; Start saves', async () => {
    await tapK('sd-settings');
    await $(testId('tv-store-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await pageRow('exe')).toMatchObject({ label: 'Launch file', sub: 'Set by Amazon Games' });
    expect(await $(testId('tv-pr-cloud')).isExisting()).toBe(false);
    await key('m');
    await waitNotice('Game settings saved');
    await waitGone('tv-store-settings');
  });

  it('Uninstall asks with no cloud sentence', async () => {
    await tapK('sd-uninstall');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(`${testId('tv-confirm')} h2`).getText()).toBe(`Uninstall ${INSTALLED}?`);
    expect(await $(`${testId('tv-confirm')} p`).getText()).toBe('Its files are deleted from this computer.');
    // B answers Cancel; then B again leaves the page.
    await key('Escape');
    await waitGone('tv-confirm');
    await key('Escape');
    await waitGone('tv-store-details');
  });

  it('Settings › Stores has an Amazon Games row first', async () => {
    await tap(testId('tv-tab-settings'));
    await tap(testId('tv-set-rail-stores'));
    await waitText('tv-pane-title', 'Stores');
    expect(await text('tv-stores-row-amazon')).toMatch(/Amazon Games[\s\S]*Signed in as sam\.plays[\s\S]*Connected/);
    const rows = (await $$('[data-testid^="tv-stores-row-"]').map((r) => r.getAttribute('data-testid'))) as string[];
    expect(rows[0]).toBe('tv-stores-row-amazon');
  });

  it('Amazon Games’s page lists nile and no Cloud saves or installer row; Disconnect… asks with the desktop’s words', async () => {
    await tap(testId('tv-stores-row-amazon'));
    await $(testId('tv-store-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-sub', /^Signed in as sam\.plays$/);
    expect(await pageRow('owned')).toMatchObject({ label: 'Games you own' });
    expect((await pageRow('tool')).value).toMatch(/^nile .+ · installed$/);
    expect((await pageRow('license')).value).toBe('nile · GPL-3.0');
    expect(await pageRow('folder')).toMatchObject({ label: 'Install folder', sub: 'New Amazon Games installs go here' });
    expect(await $(testId('tv-pr-cloud')).isExisting()).toBe(false);
    expect(await $(testId('tv-pr-installer')).isExisting()).toBe(false);
    await tap(testId('tv-pr-disconnect'));
    const d = await confirmDialog();
    expect(d).toMatchObject({ title: 'Disconnect Amazon Games?', ok: 'Disconnect', danger: true });
    expect(d.body).toMatch(/^GRID deletes your Amazon Games login and nile from this computer\. The \d+ installed games stay and still play; they can’t install or update until you connect again\.$/);
  });

  it('not connected: the note, Connect Amazon Games… on Start, the sign-in over TV mode, and B cancels silently', async () => {
    // `confirmDialog` pressed Disconnect: the page closes with the card.
    await waitNotice(/^Amazon Games disconnected( \||$)/, 10_000);
    await waitGone('tv-store-page');
    await waitText('tv-stores-row-amazon', /Not signed in[\s\S]*Not connected/);
    await tap(testId('tv-stores-row-amazon'));
    await waitText('tv-rows-sub', 'Not signed in');
    await waitText('tv-rows-main', 'Connect Amazon Games…');
    expect(await text('tv-pr-off')).toMatch(/^Sign in to Amazon Games to see the games you own in the Library and install them\. GRID installs with nile \(nile, GPL-3\.0, .+\), which it downloads when you connect\.$/);
    expect(await $(testId('tv-pr-installer')).isExisting()).toBe(false);
    await key('m');
    // nile installs first (it makes the sign-in URL), then the login window
    // opens beside TV mode; WebDriver may follow it, so every step below runs
    // in the main window.
    await browser.waitUntil(loginWindowOpen, { timeout: INSTALL_TIMEOUT, timeoutMsg: 'the login window never opened' });
    await browser.switchToWindow(MAIN_WINDOW);
    await $(testId('tv-store-login')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the sign-in layer never opened' });
    expect(await text('tv-store-login')).toContain('Sign in to Amazon Games — GRID');
    expect(await text('tv-store-login')).toContain('https://www.amazon.com/ap/signin');
    await browser
      .waitUntil(async () => JSON.stringify(await hints()) === JSON.stringify(['A Select', 'Y Keyboard', 'B Cancel sign-in']), { timeout: TRANSITION_TIMEOUT })
      .catch(() => undefined);
    expect(await hints()).toEqual(['A Select', 'Y Keyboard', 'B Cancel sign-in']);
    await key('y');
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => /^k/.test((await focused()) ?? ''), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the keyboard never took focus' });
    await key('Escape');
    await waitGone('tv-osk');
    await key('Escape');
    await waitGone('tv-store-login');
    await browser.waitUntil(async () => !(await loginWindowOpen()), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the login window stayed open' });
    await browser.switchToWindow(MAIN_WINDOW);
    expect(await browser.execute(() => document.querySelectorAll('[data-layer]').length)).toBe(0);
  });

  it('signed out with games installed, the folder shows off and Installed still lists them', async () => {
    await tap(testId('tv-tab-library'));
    await waitText('tv-lib-store-flag-amazon', 'off');
    await tap(testId('tv-lib-rail-store-amazon-installed'));
    await waitText('tv-lib-name', 'Amazon Games · Installed');
    expect(await text('tv-store-chip-off')).toBe('Not connected');
    await coverK(INSTALLED);
  });
});
