import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, FIXTURE_USERNAME, mockUrl, TRANSITION_TIMEOUT, WRONG_TOKEN } from '../helpers/env.js';
import { assertLayoutAtSizes, connectPad, focused, hints, invoke, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, chooseWelcomeLibrary, moveTo, pageErrors, testId, text, typeKeys, waitOskClosed, waitText } from '../helpers/tvDetails.js';
import { tap, tapK } from '../helpers/tvBackups.js';

const MASK = '••••••••••••';

async function oskTitle(): Promise<string> {
  await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the keyboard never opened' });
  return (await $(testId('tv-osk')).getAttribute('aria-label')) ?? '';
}

async function listTitle(): Promise<string> {
  await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the list never opened' });
  return (await $(testId('tv-list')).getAttribute('aria-label')) ?? '';
}

/** A sign-in page row as "<label> <value>". */
async function fieldText(k: string): Promise<string> {
  return (await browser.execute((id: string) => {
    const el = document.querySelector(`[data-testid="tv-rs-${id}"]`);
    return `${el?.querySelector('.lab')?.textContent ?? ''} ${el?.querySelector('.v')?.textContent ?? ''}`.trim();
  }, k)) as string;
}

/** A Choose your systems row as [name, line, emulator]. */
async function chooseText(slug: string): Promise<string[]> {
  return (await browser.execute((id: string) => {
    const el = document.querySelector(`[data-testid="tv-ch-row-${id}"]`);
    return ['.t', '.s', '.val'].map((c) => (el?.querySelector(c)?.textContent ?? '').trim());
  }, slug)) as string[];
}

async function rowKeys(sel: string): Promise<string[]> {
  return (await browser.execute((s: string) => Array.from(document.querySelectorAll(`${s} [data-k]`)).map((e) => e.getAttribute('data-k') ?? ''), sel)) as string[];
}

/** The startup box fits between the bars and the window never scrolls (item 389). */
async function expectBoxFits(): Promise<void> {
  const fit = (await browser.execute(() => {
    const box = document.querySelector('.start-box')!.getBoundingClientRect();
    const stage = document.querySelector('[data-testid="tv-stage"]')!.getBoundingClientRect();
    const se = document.scrollingElement!;
    return { inside: box.top >= stage.top - 0.5 && box.bottom <= stage.bottom + 0.5, scrolls: se.scrollHeight > se.clientHeight + 1 };
  })) as { inside: boolean; scrolls: boolean };
  expect(fit).toEqual({ inside: true, scrolls: false });
}

/**
 * Stage `tv-startup`, spec a (TV mode plan P12; romm-storefront-plan Step
 * 6): a fresh profile started with --tv. The welcome (which replaced
 * Connect) in a box that fits: Start opens the startup Menu (Switch to
 * desktop mode, Quit GRID); the library folder's browser and B. Then RomM's
 * sign-in page through the on-screen keyboard: View pastes from the
 * clipboard (seeded with the mock's non-secret address), Sign in with, the
 * button names what is missing while it is disabled, a rejected token comes
 * back with the reason; a good one connects, Continue opens Choose your
 * systems, where Start skips, and GRID opens on Library › RomM › Discover
 * with its first cover focused.
 */
describe('tv-startup (a): the welcome, RomM sign-in and Choose your systems in TV mode', () => {
  before(async () => {
    await $(testId('tv-welcome')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed the welcome' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('shows the welcome in a box that fits, with the startup top bar and hints (items 123, 132, 133, 389, 391–393)', async () => {
    expect(await $(`${testId('tv-welcome')} h1`).getText()).toBe('Welcome to GRID');
    expect(await rowKeys('#wel-rows')).toEqual(['w-lib']);
    // A fresh profile has no folder chosen yet: Next waits for one.
    expect(await text('tv-welcome-lib-value')).toBe('Choose a folder');
    expect(await $(testId('tv-welcome-next')).getAttribute('aria-disabled')).toBe('true');
    expect(await $(testId('tv-welcome-next')).getAttribute('data-why')).toBe('Choose a folder to continue.');
    // The box doesn't repeat the wordmark (391).
    expect(await $$(`${testId('tv-welcome')} .brand`).length).toBe(0);
    // Top bar: no tabs, no connection, only the clock (123).
    expect(await $(testId('tv-tabs')).isExisting()).toBe(false);
    expect(await $(testId('tv-connection')).isExisting()).toBe(false);
    expect(await $(testId('tv-clock')).isExisting()).toBe(true);
    await waitFocused('w-lib');
    // No Back (132); Menu opens the startup Menu (user decision 2026-11-10).
    expect(await hints()).toEqual(['A Choose', 'MENU Menu']);
    await expectBoxFits();
  });

  it('Start opens the startup Menu: Switch to desktop mode and Quit GRID; B closes it (user decision 2026-11-10)', async () => {
    await waitFocused('w-lib');
    await press('start');
    await $(testId('tv-menu')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Start opened no Menu on the welcome' });
    await waitFocused('gm0');
    const labels = (await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="tv-menu"] .opt')).map((e) => e.textContent?.trim() ?? ''),
    )) as string[];
    expect(labels).toEqual(['Switch to desktop mode', 'Quit GRID']);
    expect(await $(testId('tv-menu-clock')).isExisting()).toBe(true);
    expect(await $(testId('tv-menu-head')).isExisting()).toBe(false);
    expect(await hints()).toEqual(['A Select', 'B Close']);
    await press('dpad_down');
    await waitFocused('gm1');
    await press('south');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('tv-confirm')).$('h2').getText()).toBe('Quit GRID?');
    await press('east');
    await browser.waitUntil(async () => !(await $(testId('tv-layer')).isExisting()), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the Quit question never closed' });
    await press('start');
    await waitFocused('gm0');
    await press('east');
    await browser.waitUntil(async () => !(await $(testId('tv-menu')).isExisting()), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'B never closed the Menu' });
    await waitFocused('w-lib');
    expect(await $(testId('tv-welcome')).isExisting()).toBe(true);
  });

  it('Library folder opens the folder browser; B steps back to the welcome, never past it; Y uses a folder (items 97, 395)', async () => {
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder browser never opened' });
    expect(await $(`${testId('tv-folder-browser')} h1`).getText()).toBe('Library folder');
    expect(await hints()).not.toContain('B Back');
    await key('Escape');
    await $(testId('tv-welcome')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'B never came back to the welcome' });
    await waitFocused('w-lib');
    await key('Escape');
    await browser.pause(300);
    expect(await $(testId('tv-welcome')).isExisting()).toBe(true);
    expect(await text('tv-welcome-lib-value')).toBe('Choose a folder');
    await chooseWelcomeLibrary(path.join(dataDir(), 'library'));
    expect(await $(testId('tv-welcome-next')).getAttribute('aria-disabled')).toBeNull();
  });

  it("Next shows the storefronts; RomM's row opens its sign-in page, the button naming what is missing (item 397)", async () => {
    await waitFocused('w-next');
    await key('Enter');
    await $(testId('tv-welcome-stores')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Next never showed step 2' });
    await tapK('w-romm');
    await $(testId('tv-romm-signin')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the sign-in page never opened' });
    expect(await rowKeys('#rs-rows')).toEqual(['rs-url', 'rs-how', 'rs-secret']);
    expect(await fieldText('url')).toBe('Server URL https://…');
    expect(await fieldText('how')).toBe('Sign in with API token');
    expect(await fieldText('secret')).toBe('API token Paste or type your token');
    expect(await $(testId('tv-rs-go')).getAttribute('aria-disabled')).toBe('true');
    expect(await $(testId('tv-rs-go')).getAttribute('data-why')).toBe('Enter your RomM server address first.');
    await waitFocused('rs-url');
  });

  it('View pastes the clipboard into the focused field (item 396), and says so when there is nothing to paste', async () => {
    await invoke('e2e_clipboard_write', { text: '' });
    await press('select');
    await waitToast('Nothing to paste');
    expect(await text('tv-rs-url-value')).toBe('https://…');
    await invoke('e2e_clipboard_write', { text: mockUrl() });
    await press('select');
    await waitText('tv-rs-url-value', mockUrl());
    await waitToast('Pasted from the clipboard');
  });

  it('A on Server URL opens the keyboard titled "RomM server address" with its value; B keeps it (item 394)', async () => {
    await key('Enter');
    expect(await oskTitle()).toBe('RomM server address');
    expect(await text('tv-osk-value')).toBe(mockUrl());
    await key('Escape');
    await waitOskClosed();
    await waitFocused('rs-url');
    expect(await text('tv-rs-url-value')).toBe(mockUrl());
  });

  it('Sign in with switches to Username and password, which the button then names (items 393, 397)', async () => {
    await key('ArrowDown');
    await waitFocused('rs-how');
    expect(await hints()).toContain('A Change');
    expect(await hints()).toContain('VIEW Paste');
    await key('Enter');
    expect(await listTitle()).toBe('Sign in with');
    await tap(testId('tv-opt-password'));
    await waitText('tv-rs-how-value', 'Username and password');
    expect(await rowKeys('#rs-rows')).toEqual(['rs-url', 'rs-how', 'rs-user', 'rs-secret']);
    expect(await fieldText('user')).toBe('Username Your RomM user');
    expect(await fieldText('secret')).toBe('Password Your password');
    expect(await $(testId('tv-rs-go')).getAttribute('data-why')).toBe('Enter your RomM username first.');
    await waitFocused('rs-how');
    await key('Enter');
    await listTitle();
    await tap(testId('tv-opt-token'));
    await waitText('tv-rs-how-value', 'API token');
  });

  it('the API token opens masked and empty, and shows only as dots (item 394)', async () => {
    await moveTo('rs-secret', 'ArrowDown');
    await key('Enter');
    expect(await oskTitle()).toBe('API token');
    expect(await text('tv-osk-value')).toBe('');
    await typeKeys(WRONG_TOKEN);
    expect(await text('tv-osk-value')).not.toContain(WRONG_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await waitText('tv-rs-secret-value', MASK);
    expect(await text('tv-romm-signin')).not.toContain(WRONG_TOKEN);
  });

  it('a rejected token comes back with the reason above the rows (items 398, 515)', async () => {
    expect(await $(testId('tv-rs-go')).getAttribute('aria-disabled')).toBeNull();
    await tap(testId('tv-rs-go'));
    await $(testId('tv-rs-error')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'the rejected token gave no reason' });
    expect((await text('tv-rs-error')).length).toBeGreaterThan(0);
    expect(await text('tv-rs-error')).not.toContain(WRONG_TOKEN);
    expect(await $(testId('tv-choose')).isExisting()).toBe(false);
    // The token is asked for again.
    await waitText('tv-rs-secret-value', 'Paste or type your token');
  });

  it('a good token: "Connected to RomM as <user>", RomM connected on step 2, Continue opens Choose your systems (items 399, 400)', async () => {
    await waitFocused('rs-secret');
    await key('Enter');
    await typeKeys(FIXTURE_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await tap(testId('tv-rs-go'));
    await waitToast(`Connected to RomM as ${FIXTURE_USERNAME}`);
    await $(testId('tv-welcome-stores')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the sign-in page never went back' });
    await waitText('tv-welcome-row-romm-pill', 'Connected');
    await waitText('tv-welcome-go', 'Continue');
    await tapK('w-go');
    await $(testId('tv-choose')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'Choose your systems never opened' });
  });

  it('Choose your systems: the intro, a switch row per system, Install and Skip (items 400, 401, 404)', async () => {
    expect(await $(`${testId('tv-choose')} h1`).getText()).toBe('Choose your systems');
    await waitText(
      'tv-choose-intro',
      'Pick the systems you play. GRID installs an emulator for each. You can change this later in Settings › Systems.',
    );
    const glyphs = (await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="tv-choose-intro"] [data-glyph]')).map((g) => g.getAttribute('data-glyph')),
    )) as string[];
    expect(glyphs).toEqual([]);
    await $(testId('tv-ch-row-ps2')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the systems never loaded' });
    await browser.waitUntil(async () => (await rowKeys('#ch-rows')).includes('ch-windows'), { timeout: TRANSITION_TIMEOUT });
    expect(await rowKeys('#ch-rows')).toEqual(['ch-dc', 'ch-ps2', 'ch-snes', 'ch-zeebo', 'ch-windows']);
    expect(await $$('#ch-rows .sw.on').length).toBe(0);
    const ps2 = await chooseText('ps2');
    expect(ps2[0]).toBe('Sony PlayStation 2');
    expect(ps2[1]).toMatch(/^\d+ games? on RomM$/);
    expect(ps2[2]).toBe('PCSX2');
    expect((await chooseText('windows')).slice(0, 2)).toEqual(['Windows', 'Proton for Windows games']);
    expect(await text('tv-ch-val-zeebo')).toBe('No emulator available');
    expect(await text('tv-choose-install')).toBe('Install emulators');
    expect(await $(testId('tv-choose-install')).getAttribute('aria-disabled')).toBe('true');
    expect(await text('tv-choose-skip')).toBe('Skip for now');
    expect(await $(`${testId('tv-choose-skip')} [data-glyph="MENU"]`).isExisting()).toBe(true);
    await waitFocused('ch-dc');
    expect(await hints()).toEqual(['A Tick', 'X Change emulator', 'MENU Skip for now']);
    await expectBoxFits();
    // The layout sweep's checks at 720p, 1080p and 4K (P14; items 3, 4, 5).
    await assertLayoutAtSizes();
    await waitFocused('ch-dc');
  });

  it('A ticks; X changes the emulator from its own list, which ticks the system (items 402, 403)', async () => {
    await key('Enter');
    await browser.waitUntil(async () => (await $$('#ch-rows .sw.on').length) === 1, { timeout: TRANSITION_TIMEOUT });
    expect(await hints()).toEqual(['A Untick', 'X Change emulator', 'MENU Skip for now']);
    await waitText('tv-choose-install', 'Install 1 emulator');
    await key('ArrowDown');
    await waitFocused('ch-ps2');
    await key('x');
    expect(await listTitle()).toBe('Emulator for Sony PlayStation 2');
    const list = await text('tv-list');
    expect(list).toContain('Download');
    expect(list).toContain('PCSX2 (recommended)');
    expect(list).toContain('None');
    // The checked option has focus: pick it.
    await key('Enter');
    await browser.waitUntil(async () => (await $$('#ch-rows .sw.on').length) === 2, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'picking an emulator never ticked the system' });
    await waitText('tv-choose-install', 'Install 2 emulators');
    // A system nothing installable plays can't be ticked.
    await moveTo('ch-zeebo', 'ArrowDown');
    expect(await text('tv-ch-val-zeebo')).toBe('No emulator available');
    expect(await hints()).not.toContain('X Change emulator');
    await key('Enter');
    await waitToast('GRID has no emulator for Zeebo. Add your own later in Settings › Systems.');
  });

  it('Down from the last system reaches the buttons, which remember the one left (item 405)', async () => {
    await moveTo('ch-windows', 'ArrowDown');
    await key('ArrowDown');
    await waitFocused('ch-go');
    await key('ArrowRight');
    await waitFocused('ch-skip');
    await key('ArrowUp');
    await waitFocused('ch-windows');
    await key('ArrowDown');
    await waitFocused('ch-skip');
  });

  it('Start skips from anywhere; GRID opens on Library › RomM › Discover with its first cover focused (items 406, 408, 413, 417)', async () => {
    await moveTo('ch-dc', 'ArrowUp', 8);
    await press('start');
    await waitTab('library');
    expect(await $(testId('tv-choose')).isExisting()).toBe(false);
    // Discover's first cover takes focus; these fixtures may give Discover
    // no covers at all (no dates), and then the Discover row keeps it.
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => {
          const covers = document.querySelectorAll('#disc [data-cover]');
          const f = document.querySelector('[data-tv-root] .f');
          return covers.length ? f === covers[0] : f?.getAttribute('data-k') === 'r-romm:discover';
        })) as boolean,
      { timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: `Discover's first cover never took focus (focused: ${await focused()})` },
    );
    expect(await hints()).toContain('MENU Menu');
    await waitToast('Welcome to TV mode');
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
