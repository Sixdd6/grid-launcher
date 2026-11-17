import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, FIXTURE_USERNAME, mockUrl, TRANSITION_TIMEOUT, WRONG_TOKEN } from '../helpers/env.js';
import { setServerOffline } from '../helpers/openOn.js';
import { tvRailKeys, tvRailTo } from '../helpers/romm.js';
import { assertNoClipping, connectPad, focused, hints, invoke, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { tap, tapK } from '../helpers/tvBackups.js';
import { captureErrors, moveTo, pageErrors, testId, text, typeKeys, waitOskClosed, waitText } from '../helpers/tvDetails.js';

const MASK = '••••••••••••';
const host = () => new URL(mockUrl()).host;

async function rowKeys(sel: string): Promise<string[]> {
  return (await browser.execute((s: string) => Array.from(document.querySelectorAll(`${s} [data-k]`)).map((e) => e.getAttribute('data-k') ?? ''), sel)) as string[];
}

async function railKeys(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-library"] [data-rail] [data-k]')).map((e) => e.getAttribute('data-k') ?? ''),
  )) as string[];
}

/** With E2E_TV_SCREENS=1, screens are saved under ctr-out/tv-screens/romm/. */
const SHOTS = process.env.E2E_TV_SCREENS === '1' ? path.resolve(process.cwd(), '..', 'ctr-out', 'tv-screens', 'romm') : '';
async function screenshot(name: string): Promise<void> {
  if (!SHOTS) return;
  await browser.pause(400);
  mkdirSync(SHOTS, { recursive: true });
  await browser.saveScreenshot(path.join(SHOTS, `${name}.png`));
}

/**
 * Stage `tv-romm` (romm-storefront-plan Step 6; TVRommStorefront README), on
 * the Discover fixtures, started with --tv on a profile whose welcome isn't
 * done (seeded: only a library folder). The welcome's two steps, the RomM
 * sign-in page (a rejected token, then a good one), Choose your systems,
 * GRID opening on Library › All games, the shell without a Server tab
 * or connection text, Settings › Game stores with RomM between GOG and
 * Steam, the RomM page, Disconnect (the group goes: no installed RomM
 * games), Cloud saves' needs-RomM panel, and Connect RomM… again. Then
 * (folded in from the Server tab's spec, docs/tv-mode/plan.md P7) the RomM
 * group's rows, a platform's header, chips, search and X to install. The
 * seed gives SNES a default emulator. Discover, Search everything and Menu ›
 * Discover are removed for now (2026-11-17, pending the Discover rework).
 */
describe('tv-romm: RomM as a storefront in TV mode', () => {
  before(async () => {
    await $(testId('tv-welcome')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed the welcome' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('step 1: the library folder filled in, Next, the startup Menu hint', async () => {
    await screenshot('welcome-1');
    expect(await $(`${testId('tv-welcome')} h1`).getText()).toBe('Welcome to GRID');
    expect(await text('tv-welcome-lib-value')).toBe(path.join(dataDir(), 'library'));
    expect(await $(testId('tv-tabs')).isExisting()).toBe(false);
    await waitFocused('w-lib');
    expect(await hints()).toEqual(['A Choose', 'MENU Menu']);
    expect(await $(testId('tv-welcome-next')).getAttribute('aria-disabled')).toBeNull();
  });

  it('step 2: five rows in name order, Skip for now, B back to step 1', async () => {
    await moveTo('w-next', 'ArrowDown');
    await key('Enter');
    await $(testId('tv-welcome-stores')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Next never showed step 2' });
    await screenshot('welcome-2');
    expect(await rowKeys('#wel-stores')).toEqual(['w-amazon', 'w-epic', 'w-gog', 'w-romm', 'w-steam']);
    expect(await text('tv-welcome-row-romm-pill')).toBe('Not connected');
    expect(await text('tv-welcome-go')).toBe('Skip for now');
    expect(await hints()).toContain('B Back');
    await press('east');
    await $(testId('tv-welcome')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'B never went back to step 1' });
    await moveTo('w-next', 'ArrowDown');
    await key('Enter');
    await $(testId('tv-welcome-stores')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  });

  it('the RomM row opens the sign-in page; a rejected token says so and empties the field', async () => {
    await tapK('w-romm');
    await $(testId('tv-romm-signin')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the sign-in page never opened' });
    await screenshot('signin');
    expect(await text('tv-rows-title')).toBe('Connect to RomM');
    expect(await rowKeys('#rs-rows')).toEqual(['rs-url', 'rs-how', 'rs-secret']);
    expect(await $(testId('tv-rs-go')).getAttribute('data-why')).toBe('Enter your RomM server address first.');
    // View pastes into the focused field.
    await invoke('e2e_clipboard_write', { text: mockUrl() });
    await waitFocused('rs-url');
    await press('select');
    await waitText('tv-rs-url-value', mockUrl());
    await waitToast('Pasted from the clipboard');
    await moveTo('rs-secret', 'ArrowDown');
    await key('Enter');
    await typeKeys(WRONG_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await waitText('tv-rs-secret-value', MASK);
    expect(await hints()).toContain('MENU Connect');
    await press('start');
    await waitText('tv-rs-error', 'RomM rejected this token (401). Check it, or make a new one.');
    await waitFocused('rs-secret');
    expect(await text('tv-rs-secret-value')).toBe('Paste or type your token');
  });

  it('a good token connects: the card, back on step 2 with RomM connected and Continue', async () => {
    await key('Enter');
    await typeKeys(FIXTURE_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await press('start');
    await $(testId('tv-welcome-stores')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the sign-in page never went back' });
    await waitToast(`Connected to RomM as ${FIXTURE_USERNAME}`);
    await waitText('tv-welcome-row-romm-pill', 'Connected');
    await waitText('tv-welcome-row-romm-sub', `Connected as ${FIXTURE_USERNAME} · ${host()}`);
    await waitText('tv-welcome-go', 'Continue');
  });

  it('Continue (Choose your systems first when setup is needed; Start skips) opens Library › All games', async () => {
    await tapK('w-go');
    await browser.waitUntil(async () => (await $(testId('tv-choose')).isExisting()) || (await $(testId('tv-library')).isExisting()), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'Continue opened neither Choose your systems nor the Library',
    });
    if (await $(testId('tv-choose')).isExisting()) await press('start');
    await waitTab('library');
    // Discover and Open GRID on are removed for now (2026-11-17): TV mode
    // opens on All games, its rail row focused.
    await waitFocused('r-all', 'TV mode did not open on All games');
    await screenshot('all-games');
    await browser.waitUntil(async () => (await railKeys()).some((k) => k.startsWith('r-platform:')), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the RomM platform rows never appeared',
    });
    const rail = await railKeys();
    expect(rail.slice(0, 3)).toEqual(['r-all', 'r-recent', 'r-updates']);
    expect(rail).toContain('r-romm:installed');
    expect(rail).not.toContain('r-romm:discover');
    expect(rail.indexOf('fold-romm')).toBeGreaterThan(rail.indexOf('fold-drives'));
    expect(await hints()).toEqual(['A Open', 'Y Search', 'LT Sort', 'RT Select', 'MENU Menu']);
  });

  it('the shell: three tabs, no connection text, no offline line, Menu without Reconnect', async () => {
    const tabs = (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-tabs"] .tab')).map((t) => t.textContent?.trim()))) as string[];
    expect(tabs).toEqual(['Library', 'Downloads', 'Settings']);
    expect(await $(testId('tv-connection')).isExisting()).toBe(false);
    expect(await text('tv-hints-left')).not.toMatch(/Offline|No server/);
    await press('start');
    await $(testId('tv-menu')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const labels = (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-menu"] .opt')).map((e) => e.textContent?.trim() ?? ''))) as string[];
    expect(labels[0]).toBe('Downloads');
    expect(labels).not.toContain('Discover');
    expect(labels.some((l) => /Reconnect|Connect to a server/.test(l))).toBe(false);
    expect(await $(testId('tv-menu-head')).isExisting()).toBe(false);
    await press('east');
    await browser.waitUntil(async () => !(await $(testId('tv-menu')).isExisting()), { timeout: TRANSITION_TIMEOUT });
  });

  it('a platform row shows its merged grid and header', async () => {
    await press('east');
    const plat = (await railKeys()).find((k) => k.startsWith('r-platform:'))!;
    await moveTo(plat, 'ArrowDown', 20);
    await $(testId('tv-romm-platform-header')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the platform view never showed' });
    await waitText('tv-lib-count', /^\d+ games? on RomM/);
    expect(await hints()).toContain('RT Select');
    await screenshot('platform');
  });

  it('Settings › Game stores: RomM between GOG and Steam, its page, Disconnect…', async () => {
    await press('right_bumper');
    await press('right_bumper');
    await waitTab('settings');
    const rail = (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-settings"] [data-rail] [data-k]')).map((e) => e.textContent?.trim() ?? ''))) as string[];
    expect(rail).toEqual(['Appearance', 'Backup drives', 'Cloud saves', 'Game stores', 'Library', 'RetroAchievements', 'Systems', 'TV mode', 'Updates']);
    await moveTo('r-stores', 'ArrowDown');
    await waitText('tv-pane-title', 'Game stores');
    expect(await rowKeys('[data-testid="tv-pane"]')).toEqual(['sto-amazon', 'sto-epic', 'sto-gog', 'sto-romm', 'sto-steam']);
    await waitText('tv-stores-row-romm-pill', 'Connected');
    await screenshot('game-stores');
    expect(await text('tv-stores-row-romm-sub')).toBe(`Signed in as ${FIXTURE_USERNAME} · ${host()}`);
    await tapK('sto-romm');
    await $(testId('tv-romm-page')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await screenshot('romm-page');
    expect(await text('tv-rows-sub')).toBe(`Signed in as ${FIXTURE_USERNAME} · ${host()}`);
    expect(await rowKeys('#rows-page')).toEqual(['edit', 'library', 'browser', 'disconnect']);
    await tapK('disconnect');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('tv-confirm')).$('h2').getText()).toBe('Disconnect RomM?');
    await tap(testId('tv-confirm-ok'));
    await waitToast('RomM disconnected');
    await waitText('tv-rows-sub', `Not signed in · ${host()}`);
    await waitText('tv-rows-main', 'Connect RomM…');
  });

  it('with RomM disconnected and no RomM games, the Library has no RomM group', async () => {
    await press('east');
    await press('left_bumper');
    await press('left_bumper');
    await waitTab('library');
    await browser.waitUntil(async () => !(await railKeys()).includes('fold-romm'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the RomM group stayed' });
    expect((await railKeys()).some((k) => k.startsWith('r-platform:'))).toBe(false);
  });

  it("Cloud saves says it needs RomM, with Set up RomM…, which opens the RomM page; Connect RomM… signs in again", async () => {
    await press('right_bumper');
    await press('right_bumper');
    await waitTab('settings');
    // From the pane to its rail row, then up the rail.
    if (!(await focused())?.startsWith('r-')) await press('east');
    await moveTo('r-cloud-saves', 'ArrowUp');
    await $(testId('tv-needs-romm')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await screenshot('cloud-needs-romm');
    expect(await text('tv-needs-romm-text')).toMatch(/^RomM isn't connected\. GRID keeps your emulator saves/);
    await tapK('rneed');
    await $(testId('tv-romm-page')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await press('start');
    await $(testId('tv-romm-signin')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-rs-url-value')).toBe(mockUrl());
    await moveTo('rs-secret', 'ArrowDown');
    await key('Enter');
    await typeKeys(FIXTURE_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await press('start');
    await $(testId('tv-romm-page')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'connecting never went back to the RomM page' });
    await waitText('tv-rows-sub', `Signed in as ${FIXTURE_USERNAME} · ${host()}`);
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});

const SNES = 'Super Nintendo Entertainment System';

/** Types on the on-screen keyboard with the physical keyboard (keys too). */
async function typeRaw(s: string): Promise<void> {
  await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  for (const ch of s) await browser.keys(ch);
}

describe('tv-romm: a platform in Library › RomM', () => {
  before(async () => {
    // Back from the RomM page to the Library.
    await press('east');
    await $(testId('tv-romm-page')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    await tap(testId('tv-tab-library'));
    await waitTab('library');
  });

  it("the RomM group: Installed, then the platforms by name, no Discover (2026-11-17)", async () => {
    await tvRailTo('r-romm:installed');
    // The backend leaves out empty platforms (N64).
    const rail = await tvRailKeys();
    expect(rail.slice(rail.indexOf('fold-romm'))).toEqual(['fold-romm', 'r-romm:installed', 'r-platform:arcade', 'r-platform:snes']);
    expect(await text('tv-lib-rail-platform-snes')).toBe(`${SNES}5`);
    await assertNoClipping();
  });

  it('a platform: counts, chips, and Y narrows the grid as you type (items 196–206)', async () => {
    await tvRailTo('r-platform:arcade');
    await waitText('tv-lib-count', '2 games on RomM · 0 installed');
    await key('ArrowDown');
    await waitFocused('r-platform:snes');
    await waitText('tv-lib-count', '5 games on RomM · 0 installed');
    expect(await text('tv-lib-name')).toBe(SNES);
    expect(await text('tv-romm-emu')).toBe('Emulator: snes9x');
    expect(await text('tv-romm-backup-all')).toBe('Backup all 5…');
    expect(await hints()).toEqual(['A Open', 'Y Search', 'RT Select', 'MENU Menu']);
    await key('ArrowRight');
    await waitFocused('pg501');
    await assertNoClipping();
    // Up from the top row reaches the header, never the rail.
    await key('ArrowUp');
    await browser.waitUntil(async () => (await focused()) !== 'pg501', { timeout: TRANSITION_TIMEOUT });
    expect(await browser.execute(() => !!document.querySelector('[data-testid="tv-romm-platform-header"] .f'))).toBe(true);
    // The emulator chip opens the system's page (item 200).
    await waitFocused('ch-emu');
    expect(await hints()).toContain('A Open in Systems');
    await key('Enter');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the chip never opened the system page' });
    await waitText('tv-crumb', new RegExp(SNES));
    await key('Escape');
    await waitFocused('ch-emu');
    // Left and Right stay on the header's second row, firmware chip or not:
    // Right runs to Backup all and stops there, Left runs back.
    const row = async () => (await browser.execute(() => !!document.querySelector('[data-testid="tv-romm-platform-header"] .vh-row .f'))) as boolean;
    for (let i = 0; i < 3 && (await focused()) !== 'bk-all'; i++) {
      const was = await focused();
      await key('ArrowRight');
      await browser.waitUntil(async () => (await focused()) !== was, { timeout: TRANSITION_TIMEOUT, timeoutMsg: `Right from ${was} did not move` });
      expect(await row()).toBe(true);
    }
    await waitFocused('bk-all');
    await key('ArrowRight');
    await browser.pause(300);
    expect(await focused()).toBe('bk-all');
    for (let i = 0; i < 3 && (await focused()) !== 'ch-emu'; i++) {
      const was = await focused();
      await key('ArrowLeft');
      await browser.waitUntil(async () => (await focused()) !== was, { timeout: TRANSITION_TIMEOUT, timeoutMsg: `Left from ${was} did not move` });
      expect(await row()).toBe(true);
    }
    await waitFocused('ch-emu');
    await key('ArrowDown');
    await waitFocused('pg501');

    await key('y');
    await typeRaw('neon');
    await waitText('tv-lib-count', '1 of 5 games match');
    await waitText('tv-lib-search', 'Search: neon');
    await key('Enter');
    await waitOskClosed();
    expect(await browser.execute(() => Array.from(document.querySelectorAll('#lib-grid [data-k]')).map((c) => c.getAttribute('data-k')))).toEqual(['pg502']);
    await key('y');
    await typeRaw('zz');
    await waitText('tv-lib-empty', 'No games match “zz”');
    await key('Enter');
    // A new platform clears the search (item 204).
    await key('Escape');
    await waitFocused('r-platform:snes');
    await key('ArrowUp');
    await waitFocused('r-platform:arcade');
    await key('ArrowDown');
    await waitFocused('r-platform:snes');
    await waitText('tv-lib-count', '5 games on RomM · 0 installed');
    await expect($(testId('tv-lib-search'))).not.toExist();
  });

  it('X installs: the notice, then every cover of the game shows it installed (items 172, 195)', async () => {
    await key('ArrowRight');
    await waitFocused('pg501');
    await key('x');
    await waitToast('Starlit Run added to download queue');
    await browser.waitUntil(async () => browser.execute(() => !!document.querySelector('[data-testid="tv-cover-pg501"] .idot')), {
      timeout: 30_000,
      timeoutMsg: 'the installed dot never came on',
    });
    await waitText('tv-lib-count', '5 games on RomM · 1 installed');
    await browser.waitUntil(async () => (await hints()).includes('X Play'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X never became Play' });
  });

  it('raised no page errors here either', async () => {
    expect(await pageErrors()).toEqual([]);
  });

  it('leaves the mock RomM dropping every request, for part B', async () => {
    // Part B relaunches on this profile (RomM connected) and finds no answer.
    await setServerOffline(true);
  });
});
