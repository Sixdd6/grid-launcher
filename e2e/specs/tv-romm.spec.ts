import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, FIXTURE_USERNAME, mockUrl, TRANSITION_TIMEOUT, WRONG_TOKEN } from '../helpers/env.js';
import { connectPad, focused, hints, invoke, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
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

async function waitDiscoverFocus(): Promise<string> {
  let k: string | null = null;
  await browser
    .waitUntil(
      async () => {
        k = await focused();
        return k !== null && (await browser.execute((key: string) => !!document.querySelector(`#disc [data-k="${key}"]`), k));
      },
      { timeout: APP_START_TIMEOUT, timeoutMsg: 'focus never went into Discover' },
    )
    .catch((e) => {
      throw new Error(`${(e as Error).message} (focused: ${k})`);
    });
  return k!;
}

async function screenshot(name: string): Promise<void> {
  if (process.env.E2E_TV_SCREENS) await browser.saveScreenshot(path.join(dataDir(), '..', `tv-romm-${name}.png`));
}

/**
 * Stage `tv-romm` (romm-storefront-plan Step 6; TVRommStorefront README), on
 * the Discover fixtures, started with --tv on a profile whose welcome isn't
 * done (seeded: only a library folder). The welcome's two steps, the RomM
 * sign-in page (a rejected token, then a good one), Choose your systems,
 * GRID opening on Library › RomM › Discover, the shell without a Server tab
 * or connection text, Settings › Game stores with RomM between GOG and
 * Steam, the RomM page, Disconnect (the group goes: no installed RomM
 * games), Cloud saves' needs-RomM panel, and Connect RomM… again.
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

  it('Continue opens Choose your systems; Start skips; GRID opens on Library › RomM › Discover', async () => {
    await tapK('w-go');
    await $(testId('tv-choose')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'Choose your systems never opened' });
    await press('start');
    await waitTab('library');
    await waitDiscoverFocus();
    await screenshot('discover');
    const rail = await railKeys();
    expect(rail.slice(0, 3)).toEqual(['r-all', 'r-recent', 'r-updates']);
    expect(rail).toContain('r-romm:installed');
    expect(rail).toContain('r-romm:discover');
    expect(rail.some((k) => k.startsWith('r-platform:'))).toBe(true);
    expect(rail.indexOf('fold-romm')).toBeGreaterThan(rail.indexOf('fold-drives'));
    expect(await hints()).toEqual(['A Details', 'X Install', 'Y Search', 'MENU Menu']);
  });

  it('the shell: three tabs, no connection text, no offline line, Menu without Reconnect', async () => {
    const tabs = (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-tabs"] .tab')).map((t) => t.textContent?.trim()))) as string[];
    expect(tabs).toEqual(['Library', 'Downloads', 'Settings']);
    expect(await $(testId('tv-connection')).isExisting()).toBe(false);
    expect(await text('tv-hints-left')).not.toMatch(/Offline|No server/);
    await press('start');
    await $(testId('tv-menu')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const labels = (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-menu"] .opt')).map((e) => e.textContent?.trim() ?? ''))) as string[];
    expect(labels[0]).toBe('Discover');
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
    await press('rb');
    await press('rb');
    await waitTab('settings');
    const rail = (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-settings"] [data-rail] [data-k]')).map((e) => e.textContent?.trim() ?? ''))) as string[];
    expect(rail).toEqual(['Appearance', 'Backup drives', 'Cloud saves', 'Game stores', 'Library', 'RetroAchievements', 'Systems', 'TV mode', 'Updates']);
    await moveTo('r-stores', 'ArrowDown');
    await waitText('tv-pane-title', 'Game stores');
    expect(await rowKeys('[data-testid="tv-pane"]')).toEqual(['sto-amazon', 'sto-epic', 'sto-gog', 'sto-romm', 'sto-steam']);
    await waitText('tv-stores-row-romm-pill', 'Connected');
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
    await press('lb');
    await press('lb');
    await waitTab('library');
    await browser.waitUntil(async () => !(await railKeys()).includes('fold-romm'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the RomM group stayed' });
    expect((await railKeys()).some((k) => k.startsWith('r-platform:'))).toBe(false);
  });

  it("Cloud saves says it needs RomM, with Set up RomM…, which opens the RomM page; Connect RomM… signs in again", async () => {
    await press('rb');
    await press('rb');
    await waitTab('settings');
    await moveTo('r-cloud-saves', 'ArrowUp');
    await $(testId('tv-needs-romm')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
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
