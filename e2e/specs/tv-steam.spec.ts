// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, key, waitFocused, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, typeKeys, waitGone, waitOskClosed, waitText } from '../helpers/tvDetails.js';
import { byK, confirmDialog, dlLines, focusOn, listOptions, pageRow, tap, tapK, waitNotice } from '../helpers/tvBackups.js';
import { MAIN_WINDOW } from '../helpers/gogLogin.js';
import { setMarker, STEAM, waitArgv } from '../helpers/steam.js';
import { ensureSteamKey } from '../helpers/steamSetup.js';

/**
 * Stage `tv-steam` (docs/stores-phase4-plan.md package D, E2E plan and Parity
 * inventory, TV; TVStores README). On package C's Steam harness (the mock
 * Steam Web API, the fake `steam` client and the fake Steam root, seeded
 * connected as SamPlays): the Steam folder and its Available header, X
 * Install in Steam (no install page; the toast says Steam's window opens over
 * TV mode), the game page of a game Steam installs (the disabled "Installing
 * in Steam · N%", its A line and the Managed by Steam box), the Downloads
 * A-list with no Cancel, an installed game's page (Verify / Uninstall in
 * Steam, no Game settings), the Playing page with no Stop game, then
 * Settings › Stores › Steam: the account sheet, "Enter a SteamID64…" and
 * the masked key through the on-screen keyboard (never the key in clear),
 * Test results as cards, Disconnect… (confirmed) and Connect Steam. No login
 * window opens for Steam; WebDriver stays on the main window. Green only once
 * packages A and C are merged.
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

describe('tv-steam: Steam in TV mode', () => {
  const INSTALL = 'Harbor of Bells';
  const INSTALLED = 'Moonlit Ferry';

  before(async () => {
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
    // The seed writes no key: save it through the app's own `steam_save`
    // (reversed on the wire, helpers/steamSetup.ts).
    await ensureSteamKey();
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await connectPad(XBOX_PAD);
  });

  after(async () => {
    // Steam opens no window of GRID's: WebDriver never left the main window.
    expect(await browser.getWindowHandle()).toBe(MAIN_WINDOW);
    expect(await pageErrors()).toEqual([]);
  });

  it('the rail has a Steam folder last, with Installed and Available', async () => {
    await tap(testId('tv-tab-library'));
    await $(testId('tv-lib-fold-store-steam')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no Steam folder in the Library rail' });
    expect(await text('tv-lib-fold-store-steam')).toContain('Steam');
    // Connected, the client found and the key working: no flag.
    expect(await $(testId('tv-lib-store-flag-steam')).isExisting()).toBe(false);
    await $(testId('tv-lib-rail-store-steam-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('tv-lib-rail-store-steam-available')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const folds = (await $$('[data-testid^="tv-lib-fold-store-"]').map((f) => f.getAttribute('data-testid'))) as string[];
    expect(folds[folds.length - 1]).toBe('tv-lib-fold-store-steam');
  });

  it('Available has the Steam account chip and Refresh; X installs in Steam with no install page', async () => {
    await tap(testId('tv-lib-rail-store-steam-available'));
    await waitText('tv-lib-name', 'Steam · Available');
    await waitText('tv-lib-count', /^\d+ games? you own and haven’t installed · updated /);
    expect(await text('tv-store-chip-account')).toBe(`Steam account: ${STEAM.persona}`);
    expect(await $(testId('tv-store-chip-cloud')).isExisting()).toBe(false);
    expect(await $(testId('tv-store-refresh')).isExisting()).toBe(true);
    expect(await $(testId('tv-store-installer')).isExisting()).toBe(false);
    const k = await coverK(INSTALL);
    await focusOn(byK(k));
    await browser.waitUntil(async () => (await hints()).includes('X Install in Steam'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X Install in Steam never showed' });
    await key('x');
    await waitNotice(`Steam opened to install ${INSTALL} | Choose the library folder there; GRID shows the progress. Steam’s window opens over TV mode.`, 10_000);
    expect(await $(testId('tv-store-install')).isExisting()).toBe(false);
    await waitArgv(/steam:\/\/install\/\d+/);
  });

  it('the game page of a game Steam installs: the disabled Installing in Steam with A’s line, and the Managed by Steam box', async () => {
    const k = await coverK(INSTALL);
    await focusOn(byK(k));
    // While Steam installs, X does nothing on the cover.
    await browser.waitUntil(async () => !(await hints()).some((h) => h.startsWith('X ')), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X stayed on a cover Steam installs' });
    expect(await $(`${byK(k)} [data-testid="tv-store-cover-bar"]`).getAttribute('class')).toContain('managed');
    // Steam's download has started: the cover's percentage, never "Waiting".
    await browser.waitUntil(async () => /^\d+%$/.test(((await $(`${byK(k)} [data-testid="tv-store-cover-job"]`).getText().catch(() => '')) as string).trim()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the cover never showed the install’s percentage',
    });
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-det-title', INSTALL);
    await browser.waitUntil(async () => /^Installing in Steam · \d+%$/.test((await buttons())[0] ?? ''), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the first button never read Installing in Steam · N%',
    });
    const first = $(testId('tv-store-det-install'));
    expect(await first.getAttribute('data-why')).toBe('Managed by Steam. Pause or cancel it in Steam.');
    expect(await text('tv-store-managed')).toMatch(/^Managed by Steam\. Pause or cancel it in Steam\./);
    // The page opens on its first enabled item, never the disabled button.
    expect(await focused()).not.toBe('sd-install');
    expect(await buttons()).not.toContain('Game settings');
    await key('Escape');
    await waitGone('tv-store-details');
  });

  it('Downloads: "Installing in Steam N% · Managed by Steam", and A lists no Cancel', async () => {
    await tap(testId('tv-tab-downloads'));
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(INSTALL)), { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${INSTALL} has no Downloads row` });
    const mine = (await dlLines()).find((r) => r.title.startsWith(INSTALL))!;
    expect(mine.kind).toContain('Steam');
    expect(mine.line).toMatch(/^(Installing in Steam \d+% · Managed by Steam|Installed by Steam)$/);
    if (/^Installing/.test(mine.line)) {
      const row = (await browser.execute(
        (t: string) => Array.from(document.querySelectorAll('[data-testid^="tv-dl-row-"]')).find((r) => (r.textContent ?? '').includes(t))?.getAttribute('data-k') ?? null,
        INSTALL,
      )) as string;
      await tap(byK(row));
      await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
      expect(await text('tv-list')).toContain('Managed by Steam: pause or cancel it in Steam');
      expect(await $(testId('tv-dl-act-cancel')).isExisting()).toBe(false);
      expect(await $(testId('tv-dl-act-retry')).isExisting()).toBe(false);
      await key('Escape');
      await waitGone('tv-list');
    }
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(INSTALL) && r.line === 'Installed by Steam'), {
      timeout: 30_000,
      timeoutMsg: `${INSTALL} never read Installed by Steam`,
    });
  });

  it('a cancelled Steam window: the row waits for Steam; A "Stop waiting" lists Stop waiting and Open; focus stays on the ended row', async () => {
    // The user closes Steam's install window: the fake writes no manifest
    // (TVStores README "Steam: waiting for Steam to start", 2026-11-14).
    const WAIT = 'Signal Lost';
    setMarker('.fake-install-cancel');
    try {
      await tap(testId('tv-tab-library'));
      await tap(testId('tv-lib-rail-store-steam-available'));
      const k = await coverK(WAIT);
      await focusOn(byK(k));
      await browser.waitUntil(async () => (await hints()).includes('X Install in Steam'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X Install in Steam never showed' });
      await key('x');
      await waitNotice(`Steam opened to install ${WAIT} | Choose the library folder there; GRID shows the progress. Steam’s window opens over TV mode.`, 10_000);
      await browser.waitUntil(async () => ((await $(`${byK(k)} [data-testid="tv-store-cover-job"]`).getText().catch(() => '')) as string).trim() === 'Waiting', {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the cover never read Waiting',
      });
      await tap(testId('tv-tab-downloads'));
      await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(WAIT)), { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${WAIT} has no Downloads row` });
      const rowK = (await browser.execute(
        (t: string) => Array.from(document.querySelectorAll('[data-testid^="tv-dl-row-"]')).find((r) => (r.textContent ?? '').includes(t))?.getAttribute('data-k') ?? null,
        WAIT,
      )) as string;
      expect((await dlLines()).find((r) => r.title.startsWith(WAIT))!.line).toBe('Waiting for Steam to start the download');
      // No bar and no sparkline while it waits.
      expect(await $(`${byK(rowK)} .pbar`).isExisting()).toBe(false);
      expect(await $(`${byK(rowK)} .spark`).isExisting()).toBe(false);
      await focusOn(byK(rowK));
      await browser.waitUntil(async () => (await hints()).includes('A Stop waiting'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'A never read Stop waiting' });
      await key('Enter');
      await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
      expect(await listOptions()).toEqual({ title: WAIT, options: ['Stop waiting', `Open ${WAIT}`] });
      expect(await $(testId('tv-list')).$('.sub').getText()).toBe(
        'Steam · Waiting for Steam to start the download. If you cancelled it in Steam, stop waiting. If Steam installs it later, GRID still adds it.',
      );
      await tap(testId('tv-dl-act-stop_waiting'));
      await waitNotice(`Stopped waiting for Steam | If Steam installs ${WAIT} later, GRID adds it.`);
      await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(WAIT) && r.line === 'Not installed · you stopped waiting for Steam'), {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `${WAIT}’s row never ended "Not installed"`,
      });
      // Focus stays on the row, now in Completed, with A: Dismiss.
      expect(await $(`[data-testid="tv-dl-seg-completed"] [data-k="${rowK}"]`).isExisting()).toBe(true);
      await waitFocused(rowK, 'focus left the ended row');
      await browser.waitUntil(async () => (await hints()).includes('A Dismiss'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'A never read Dismiss' });
      expect(await $(`${byK(rowK)} .s.err`).isExisting()).toBe(false);
      await key('Enter');
      await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
      expect((await listOptions()).options).toEqual(['Dismiss', `Open ${WAIT}`]);
      expect(await $(testId('tv-list')).$('.sub').getText()).toBe('Steam');
      await key('Escape');
      await waitGone('tv-list');
    } finally {
      setMarker('.fake-install-cancel', false);
    }
  });

  it('the game page while Steam waits: primary Stop waiting for Steam and the box; after it, focus lands on Install in Steam', async () => {
    const WAIT = 'Quiet Comet';
    setMarker('.fake-install-cancel');
    try {
      await tap(testId('tv-tab-library'));
      await tap(testId('tv-lib-rail-store-steam-available'));
      const k = await coverK(WAIT);
      await focusOn(byK(k));
      await browser.waitUntil(async () => (await hints()).includes('X Install in Steam'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X Install in Steam never showed' });
      await key('x');
      await waitNotice(`Steam opened to install ${WAIT} | Choose the library folder there; GRID shows the progress. Steam’s window opens over TV mode.`, 10_000);
      await key('Enter');
      await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
      await waitText('tv-det-title', WAIT);
      await browser.waitUntil(async () => (await buttons())[0] === 'Stop waiting for Steam', {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the first button never read Stop waiting for Steam',
      });
      expect(await buttons()).toEqual(['Stop waiting for Steam']);
      expect(await text('tv-store-managed')).toBe(
        'Waiting for Steam to start the download. Steam’s install window is open. If you cancelled it there, stop waiting.',
      );
      expect(await $('[data-testid="tv-store-managed"] .pbar').isExisting()).toBe(false);
      expect(((await $('[data-testid="tv-store-det-cover"] [data-testid="tv-store-cover-job"]').getText()) as string).trim()).toBe('Waiting');
      await tap(testId('tv-store-det-stop_waiting'));
      await waitNotice(`Stopped waiting for Steam | If Steam installs ${WAIT} later, GRID adds it.`);
      await browser.waitUntil(async () => (await buttons())[0] === 'Install in Steam', {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'Install in Steam never came back',
      });
      await waitFocused('sd-install', 'focus never landed on Install in Steam');
      expect(await $(testId('tv-store-managed')).isExisting()).toBe(false);
      await key('Escape');
      await waitGone('tv-store-details');
      await tap(testId('tv-tab-downloads'));
      await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(WAIT) && r.line === 'Not installed · you stopped waiting for Steam'), {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `${WAIT}’s row never ended "Not installed"`,
      });
    } finally {
      setMarker('.fake-install-cancel', false);
    }
  });

  it('an installed game’s page: Play, Verify in Steam, Uninstall in Steam…; no Game settings or Cloud saves', async () => {
    await tap(testId('tv-tab-library'));
    await tap(testId('tv-lib-rail-store-steam-installed'));
    await waitText('tv-lib-name', 'Steam · Installed');
    await waitText('tv-lib-count', /^\d+ games? · in your Steam libraries$/);
    const k = await coverK(INSTALLED);
    await focusOn(byK(k));
    await browser.waitUntil(async () => (await hints()).includes('X Play'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X Play never showed' });
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await buttons()).toEqual(['Play', 'Verify in Steam', 'Uninstall in Steam…']);
    await waitFocused('sd-play');
    expect(await text('tv-store-line-runs')).toBe('Launches through Steam');
    expect(await text('tv-store-line-nobk')).toBe(STEAM.noBackup);
    expect(await $(testId('tv-store-tab-Cloud saves')).isExisting()).toBe(false);
  });

  it('Play: "Starting … through Steam…", the Playing page says Steam runs it and has no Stop game; it closes when the game ends', async () => {
    await tapK('sd-play');
    await waitNotice(`Starting ${INSTALLED} through Steam…`, 10_000);
    await $(testId('tv-store-playing')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-playing-line', /^Steam · running in Steam · \d+:\d\d$/);
    expect(await text('tv-store-playing')).toContain('Steam runs this game. Quit it from the game; GRID notices when it closes.');
    expect(await $(testId('tv-playing-stop')).isExisting()).toBe(false);
    // The fake runs the game for E2E_STEAM_RUN_SECS (4 s); the watch then
    // needs two misses in a row (2.5 s apart) to end the session.
    await browser.waitUntil(async () => !(await $(testId('tv-store-playing')).isExisting()), {
      timeout: 20_000,
      timeoutMsg: 'the Playing page stayed after Steam ended the game',
    });
    await key('Escape');
    await waitGone('tv-store-details');
  });

  it('Settings › Stores has the Steam row last: SteamID64 and Connected', async () => {
    await tap(testId('tv-tab-settings'));
    await tap(testId('tv-set-rail-stores'));
    await waitText('tv-pane-title', 'Stores');
    expect(await text('tv-stores-row-steam')).toMatch(new RegExp(`Steam[\\s\\S]*SteamID64 ${STEAM.steamId}[\\s\\S]*Connected`));
    const rows = (await $$('[data-testid^="tv-stores-row-"]').map((r) => r.getAttribute('data-testid'))) as string[];
    expect(rows[rows.length - 1]).toBe('tv-stores-row-steam');
  });

  it('Steam’s page: the client, the account sheet, the key as dots, Test as cards', async () => {
    await tap(testId('tv-stores-row-steam'));
    await $(testId('tv-steam-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect((await pageRow('client')).value).toMatch(/^Found · .+$/);
    expect((await pageRow('account')).value).toBe(`${STEAM.persona} · ${STEAM.steamId}`);
    expect((await pageRow('key')).value).toMatch(/^•+$/);
    expect(await text('tv-steam-page')).not.toContain(STEAM.accountName);
    expect(await hints()).toEqual(expect.arrayContaining(['MENU Save']));
    // The account sheet: the local accounts by persona, then Enter a SteamID64….
    await tap(testId('tv-pr-account'));
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const sheet = await listOptions();
    expect(sheet.title).toBe('Steam account');
    expect(sheet.options.some((o) => o.startsWith(`${STEAM.persona} · ${STEAM.steamId}`))).toBe(true);
    expect(sheet.options.some((o) => o.startsWith('couchguest'))).toBe(true);
    expect(sheet.options.some((o) => o.startsWith('Enter a SteamID64…'))).toBe(true);
    await tap(testId('tv-steam-acct-manual'));
    await typeKeys('12345');
    await key('Enter');
    await waitOskClosed();
    expect((await pageRow('account')).value).toBe('12345');
    // A SteamID64 of the wrong form is refused before any request.
    await tap(testId('tv-pr-test'));
    await waitNotice('That isn’t a SteamID64 | It has 17 digits and starts with 7656.', 10_000);
    // Back to SamPlays; a revoked key through the masked keyboard is rejected, with no status code.
    await tap(testId('tv-pr-account'));
    await tap(testId(`tv-steam-acct-${STEAM.steamId}`));
    await tap(testId('tv-pr-key'));
    await typeKeys(STEAM.revokedKey);
    // The masked keyboard never shows the key in clear.
    expect(await text('tv-osk-value')).toMatch(/^•+$/);
    await key('Enter');
    await waitOskClosed();
    expect((await pageRow('key')).value).toMatch(/^•+$/);
    expect(await text('tv-steam-page')).not.toContain(STEAM.revokedKey);
    await tap(testId('tv-pr-test'));
    await waitNotice('Steam rejected the key | Check that you entered all 32 characters, or make a new key.', 10_000);
  });

  it('Disconnect… asks with the desktop’s words; the card says the key was deleted', async () => {
    await tap(testId('tv-pr-disconnect'));
    const d = await confirmDialog();
    expect(d).toMatchObject({
      title: 'Disconnect Steam?',
      body: 'GRID forgets your SteamID64 and deletes your Web API key from the OS keyring. Steam games leave the Library; they stay installed in Steam.',
      ok: 'Disconnect',
      danger: true,
    });
    await waitNotice('Steam disconnected | Your key was deleted from the keyring', 10_000);
    await waitGone('tv-steam-page');
    await waitText('tv-stores-row-steam', /Not set up[\s\S]*Not connected/);
  });

  it('not connected: Connect Steam on Start with the key through the keyboard', async () => {
    await tap(testId('tv-stores-row-steam'));
    await $(testId('tv-steam-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-main', 'Connect Steam');
    expect(await $(testId('tv-pr-disconnect')).isExisting()).toBe(false);
    expect((await pageRow('key')).value).toBe('Not set');
    await tap(testId('tv-pr-key'));
    await typeKeys(STEAM.key);
    await key('Enter');
    await waitOskClosed();
    await key('m');
    await waitNotice(/^Steam connected · \d+ games owned/, 15_000);
    await waitText('tv-rows-main', 'Save');
    expect(await text('tv-steam-page')).not.toContain(STEAM.key);
  });
});
