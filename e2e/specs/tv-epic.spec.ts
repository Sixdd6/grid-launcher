// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, INSTALL_TIMEOUT, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, key, waitFocused, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, confirmDialog, dlLines, focusOn, pageRow, tap, tapK, waitNotice } from '../helpers/tvBackups.js';
import { loginWindowOpen, MAIN_WINDOW } from '../helpers/gogLogin.js';

/**
 * Stage `tv-epic` (docs/stores-phase2-plan.md package E, E2E plan; TVStores
 * README § Library, § The game page, § Settings › Stores, § Signing in). On
 * package C's Epic harness: the mock Epic server, the fake legendary, and
 * `seed/epic-seed.mjs` (signed in as SamPlays; Starlight Courier and Vector
 * Bloom installed; the prototype's six owned games, Iron Orchard needing the
 * EA app). Enters TV mode, opens Library › Epic Games, checks the Not
 * supported page, installs a game through the install page (no Version or
 * Language rows), follows it in Downloads, opens an installed game's tabs
 * and Game settings, then Settings › Stores › Epic Games: its rows, the
 * Disconnect confirm and the sign-in over TV mode. Green only once the
 * backend packages (A, B, C) and D are merged.
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

describe('tv-epic: Epic Games in TV mode', () => {
  const INSTALL = 'Glimmer Deep';
  const NOT_SUPPORTED = 'Iron Orchard';
  const INSTALLED = 'Starlight Courier';

  before(async () => {
    // The Epic seed may start with or without a RomM server: connect when asked.
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

  it('the rail has an Epic Games folder with Installed and Available, open by default', async () => {
    // The tabs wrap (LB/RB), so go to the tab itself.
    await tap(testId('tv-tab-library'));
    await $(testId('tv-lib-fold-store-epic')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no Epic Games folder in the Library rail' });
    expect(await $(testId('tv-lib-fold-store-epic')).getAttribute('aria-expanded')).toBe('true');
    expect(await text('tv-lib-fold-store-epic')).toContain('Epic Games');
    // Connected with legendary installed: no `!` or `off` flag.
    expect(await $(testId('tv-lib-store-flag-epic')).isExisting()).toBe(false);
    await $(testId('tv-lib-rail-store-epic-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('tv-lib-rail-store-epic-available')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // The rail's store folders are alphabetical: Epic Games before GOG when both show.
    const folds = (await $$('[data-testid^="tv-lib-fold-store-"]').map((f) => f.getAttribute('data-testid'))) as string[];
    if (folds.includes('tv-lib-fold-store-gog')) expect(folds.indexOf('tv-lib-fold-store-epic')).toBeLessThan(folds.indexOf('tv-lib-fold-store-gog'));
  });

  it('Available has the two-row header with Refresh and no installer button or Select', async () => {
    await tap(testId('tv-lib-rail-store-epic-available'));
    await waitText('tv-lib-name', 'Epic Games · Available');
    await waitText('tv-lib-count', /^\d+ games? you own and haven’t installed · updated /);
    expect(await text('tv-store-chip-account')).toBe('Signed in as SamPlays');
    expect(await text('tv-store-chip-cloud')).toBe('Cloud saves: on');
    expect(await $(testId('tv-store-installer')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-backup-all')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-refresh')).isExisting()).toBe(true);
    expect(await hints()).not.toEqual(expect.arrayContaining(['RT Select']));
  });

  it('a game that needs the EA app: NOT SUPPORTED, no X, and a disabled Not supported that says why', async () => {
    const k = await coverK(NOT_SUPPORTED);
    expect(await browser.execute((key: string) => document.querySelector(`[data-k="${key}"] [data-testid="tv-store-cover-tag"]`)?.textContent?.trim() ?? '', k)).toBe('NOT SUPPORTED');
    await focusOn(byK(k));
    expect((await hints()).some((h) => h.startsWith('X '))).toBe(false);
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the game page never opened' });
    await waitText('tv-det-title', NOT_SUPPORTED);
    expect(await text('tv-det-header')).toMatch(/^Epic Games/);
    expect(await buttons()).toEqual(['Not supported']);
    expect(await text('tv-store-line-ns')).toBe('Needs the EA app. GRID can’t install games that start through another company’s launcher.');
    expect(await text('tv-store-line-nobk')).toBe('Epic Games games can’t be backed up: legendary installs only from Epic’s servers.');
    // A on the disabled button says why, as a card. The page opens on its
    // first enabled item (a disabled one gives way, TV focus rule item 78),
    // so go to the button itself.
    await focusOn(byK('sd-install'));
    await key('Enter');
    await waitNotice(/^Needs the EA app\./, 10_000);
    await key('Escape');
    await waitGone('tv-store-details');
  });

  it('Install… opens the install page with no Version or Language rows; Start queues it', async () => {
    const k = await coverK(INSTALL);
    await focusOn(byK(k));
    expect(await hints()).toEqual(expect.arrayContaining(['X Install…']));
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-store-line-runs')).toMatch(/^Windows only · runs through .+$/);
    expect(await $(testId('tv-store-tab-Cloud saves')).isExisting()).toBe(false);
    await waitFocused('sd-install');
    await key('Enter');
    await $(testId('tv-store-install')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the install page never opened' });
    await waitText('tv-rows-title', `Install ${INSTALL}`);
    await waitText('tv-rows-sub', /^Epic Games · .+ download$/);
    await waitText('tv-rows-summary', /^.+ to download · .+ free on .+$/);
    // Windows only, one language: no Version radios, no Language list.
    expect(await $(testId('tv-pr-ver-linux')).isExisting()).toBe(false);
    expect(await $(testId('tv-pr-ver-windows')).isExisting()).toBe(false);
    expect(await $(testId('tv-pr-lang')).isExisting()).toBe(false);
    expect(await pageRow('compat')).toMatchObject({ label: 'Compatibility tool', sub: 'The default from Settings › Systems › Windows' });
    expect(await pageRow('folder')).toMatchObject({ label: 'Install folder', sub: 'The default for Epic Games games is in Settings › Stores' });
    expect(await hints()).toEqual(expect.arrayContaining(['MENU Install']));
    await key('m');
    await waitGone('tv-store-install');
    await waitNotice(`${INSTALL} added to download queue`, 10_000);
  });

  it('Downloads shows the store row with the Epic Games badge until it is installed', async () => {
    // The game page is still open under the install page: go to the tab itself.
    await tap(testId('tv-tab-downloads'));
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(INSTALL)), { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${INSTALL} has no Downloads row` });
    const mine = (await dlLines()).find((r) => r.title.startsWith(INSTALL))!;
    expect(mine.kind).toContain('Epic Games');
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(INSTALL) && /^Installed/.test(r.line)), {
      timeout: INSTALL_TIMEOUT * 3,
      timeoutMsg: `${INSTALL} never finished installing`,
    });
    await waitNotice(new RegExp(`^${INSTALL} installed`), 10_000);
  });

  it('Installed: no Backup all or Select; an installed game has Play and no Backup to drive…', async () => {
    await tap(testId('tv-tab-library'));
    await tap(testId('tv-lib-rail-store-epic-installed'));
    await waitText('tv-lib-name', 'Epic Games · Installed');
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
    expect(await text('tv-store-line-runs')).toMatch(/^Runs through .+ · Windows version$/);
    expect(await text('tv-store-line-nobk')).toBe('Epic Games games can’t be backed up: legendary installs only from Epic’s servers.');
    expect(await $(testId('tv-store-line-on')).isExisting()).toBe(false);
  });

  it('LB / RB reach Cloud saves, which syncs with Epic Games', async () => {
    await $(testId('tv-store-tab-Cloud saves')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Overview → Media → Cloud saves; RB once more wraps back to Overview.
    await key('e');
    await key('e');
    await $(testId('tv-store-cloud')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-store-cloud')).toContain('GRID syncs with Epic Games before the game starts and after it closes');
    // The RomM note sits under Save location, after the cloud rows (TVStores
    // prototype), so it is in the tab's body rather than its rows.
    expect(await text('tv-det-body')).toContain('RomM cloud saves are off for store games.');
    await key('e');
    await waitGone('tv-store-cloud');
  });

  it('Game settings says the launch file is set by Epic Games; Start saves', async () => {
    await tapK('sd-settings');
    await $(testId('tv-store-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await pageRow('exe')).toMatchObject({ label: 'Launch file', sub: 'Set by Epic Games' });
    expect(await pageRow('cloud')).toMatchObject({ label: 'Cloud saves' });
    await key('m');
    await waitNotice('Game settings saved');
    await waitGone('tv-store-settings');
  });

  it('Uninstall asks with the Epic words', async () => {
    await tapK('sd-uninstall');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(`${testId('tv-confirm')} h2`).getText()).toBe(`Uninstall ${INSTALLED}?`);
    expect(await $(`${testId('tv-confirm')} p`).getText()).toBe('Its files are deleted from this computer. Its cloud saves stay in your Epic Games account.');
    // B answers Cancel; then B again leaves the page.
    await key('Escape');
    await waitGone('tv-confirm');
    await key('Escape');
    await waitGone('tv-store-details');
  });

  it('Settings › Stores has an Epic Games row before GOG’s', async () => {
    await tap(testId('tv-tab-settings'));
    await tap(testId('tv-set-rail-stores'));
    await waitText('tv-pane-title', 'Stores');
    expect(await text('tv-stores-row-epic')).toMatch(/Epic Games[\s\S]*Signed in as SamPlays[\s\S]*Connected/);
    const rows = (await $$('[data-testid^="tv-stores-row-"]').map((r) => r.getAttribute('data-testid'))) as string[];
    if (rows.includes('tv-stores-row-gog')) expect(rows.indexOf('tv-stores-row-epic')).toBeLessThan(rows.indexOf('tv-stores-row-gog'));
  });

  it('Epic Games’s page lists legendary and no installer row; Disconnect… asks with the desktop’s words', async () => {
    await tap(testId('tv-stores-row-epic'));
    await $(testId('tv-store-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-sub', /^Signed in as SamPlays$/);
    expect(await pageRow('owned')).toMatchObject({ label: 'Games you own' });
    expect((await pageRow('tool')).value).toMatch(/^legendary .+ · installed$/);
    expect((await pageRow('license')).value).toBe('legendary · GPL-3.0');
    expect(await pageRow('folder')).toMatchObject({ label: 'Install folder', sub: 'New Epic Games installs go here' });
    expect(await pageRow('cloud')).toMatchObject({ label: 'Cloud saves', on: true });
    expect(await $(testId('tv-pr-installer')).isExisting()).toBe(false);
    await tap(testId('tv-pr-disconnect'));
    const d = await confirmDialog();
    expect(d).toMatchObject({ title: 'Disconnect Epic Games?', ok: 'Disconnect', danger: true });
    expect(d.body).toMatch(/^GRID deletes your Epic Games login and legendary from this computer\. The \d+ installed games stay and still play; they can’t install, update or sync cloud saves until you connect again\.$/);
  });

  it('not connected: the note, Connect Epic Games… on Start, the sign-in over TV mode, and B cancels silently', async () => {
    // `confirmDialog` pressed Disconnect: the page closes with the card.
    await waitNotice(/^Epic Games disconnected( \||$)/, 10_000);
    await waitGone('tv-store-page');
    await waitText('tv-stores-row-epic', /Not signed in[\s\S]*Not connected/);
    await tap(testId('tv-stores-row-epic'));
    await waitText('tv-rows-sub', 'Not signed in');
    await waitText('tv-rows-main', 'Connect Epic Games…');
    expect(await text('tv-pr-off')).toMatch(/^Sign in to Epic Games to see the games you own in the Library and install them\. GRID installs with legendary \(legendary, GPL-3\.0, .+\), which it downloads when you connect\.$/);
    expect(await $(testId('tv-pr-installer')).isExisting()).toBe(false);
    await key('m');
    // The login window opens beside TV mode; WebDriver may follow it, so
    // every step below runs in the main window.
    await browser.waitUntil(loginWindowOpen, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the login window never opened' });
    await browser.switchToWindow(MAIN_WINDOW);
    await $(testId('tv-store-login')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the sign-in layer never opened' });
    expect(await text('tv-store-login')).toContain('Sign in to Epic Games — GRID');
    expect(await text('tv-store-login')).toContain('https://www.epicgames.com/id/login');
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
    await waitText('tv-lib-store-flag-epic', 'off');
    await tap(testId('tv-lib-rail-store-epic-installed'));
    await waitText('tv-lib-name', 'Epic Games · Installed');
    expect(await text('tv-store-chip-off')).toBe('Not connected');
    await coverK(INSTALLED);
  });
});
