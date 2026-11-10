import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT, WRONG_TOKEN } from '../helpers/env.js';
import { connectPad, focused, hints, invoke, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, pageErrors, testId, text, typeKeys, waitOskClosed, waitText } from '../helpers/tvDetails.js';

const MASK = '••••••••••••';

async function oskTitle(): Promise<string> {
  await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the keyboard never opened' });
  return (await $(testId('tv-osk')).getAttribute('aria-label')) ?? '';
}

async function listTitle(): Promise<string> {
  await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the list never opened' });
  return (await $(testId('tv-list')).getAttribute('aria-label')) ?? '';
}

/** A Connect row as "<label> <value>". */
async function fieldText(k: string): Promise<string> {
  return (await browser.execute((id: string) => {
    const el = document.querySelector(`[data-testid="tv-cf-${id}"]`);
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
 * Stage `tv-startup`, spec a (TV mode plan P12): a fresh profile started
 * with --tv. Connect through the on-screen keyboard: Start presses Connect
 * and names what is missing while it is disabled, View pastes from the
 * clipboard (seeded with the mock's non-secret address and the library
 * path), the folder browser and B, a rejected token comes back with the
 * reason, then Choose your systems, where Start skips, and GRID opens on
 * Server › Discover with its first cover focused.
 */
describe('tv-startup (a): Connect and Choose your systems in TV mode', () => {
  const library = path.join(dataDir(), 'library');

  before(async () => {
    await $(testId('tv-connect')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed Connect' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('shows Connect in a box that fits, with the startup top bar and hints (items 123, 132, 133, 389, 391–393)', async () => {
    await waitText('tv-connect', /^Connect to your RomM server/);
    expect(await $(`${testId('tv-connect')} h1`).getText()).toBe('Connect to your RomM server');
    expect(await $(`${testId('tv-connect')} .sub`).getText()).toBe('Everything here works with a controller. A keyboard works too.');
    expect(await rowKeys('#cf-rows')).toEqual(['cf-url', 'cf-how', 'cf-token', 'cf-lib']);
    expect(await fieldText('url')).toBe('Server https://…');
    expect(await fieldText('how')).toBe('Sign in with API token');
    expect(await fieldText('token')).toBe('API token Paste or type your token');
    expect(await fieldText('lib')).toMatch(/^Library folder/);
    // The box doesn't repeat the wordmark (391).
    expect(await $$(`${testId('tv-connect')} .brand`).length).toBe(0);
    // Top bar: no tabs, no connection, only the clock (123).
    expect(await $(testId('tv-tabs')).isExisting()).toBe(false);
    expect(await $(testId('tv-connection')).isExisting()).toBe(false);
    expect(await $(testId('tv-clock')).isExisting()).toBe(true);
    await waitFocused('cf-url');
    // No Back and no Menu (132, 133).
    expect(await hints()).toEqual(['A Edit', 'VIEW Paste']);
    await expectBoxFits();
  });

  it('Start names what is missing while Connect is disabled, and opens no Menu (item 367, 397; ruling 6)', async () => {
    expect(await $(testId('tv-connect-go')).getAttribute('aria-disabled')).toBe('true');
    await press('start');
    await waitToast('Fill in the server, the API token and the library folder first.');
    expect(await $(testId('tv-menu')).isExisting()).toBe(false);
    expect(await $(testId('tv-layer')).isExisting()).toBe(false);
  });

  it('View pastes the clipboard into the focused field (item 396)', async () => {
    await invoke('e2e_clipboard_write', { text: mockUrl() });
    await press('select');
    await waitText('tv-cf-url-value', mockUrl());
    await waitToast('Pasted from the clipboard');
  });

  it('A on Server opens the keyboard titled "Server address" with its value; B keeps it (item 394)', async () => {
    await key('Enter');
    expect(await oskTitle()).toBe('Server address');
    expect(await text('tv-osk-value')).toBe(mockUrl());
    await key('Escape');
    await waitOskClosed();
    await waitFocused('cf-url');
    expect(await text('tv-cf-url-value')).toBe(mockUrl());
  });

  it('Sign in with switches to Username and password, which Connect then names (items 393, 397)', async () => {
    await key('ArrowDown');
    await waitFocused('cf-how');
    expect(await hints()).toEqual(['A Change', 'VIEW Paste']);
    await key('Enter');
    expect(await listTitle()).toBe('Sign in with');
    await $(testId('tv-opt-password')).click();
    await waitText('tv-cf-how-value', 'Username and password');
    expect(await rowKeys('#cf-rows')).toEqual(['cf-url', 'cf-how', 'cf-user', 'cf-password', 'cf-lib']);
    expect(await fieldText('user')).toBe('Username Your RomM user');
    expect(await fieldText('password')).toBe('Password Your password');
    await press('start');
    await waitToast('Fill in the server, the username and password and the library folder first.');
    await waitFocused('cf-how');
    await key('Enter');
    await listTitle();
    await $(testId('tv-opt-token')).click();
    await waitText('tv-cf-how-value', 'API token');
  });

  it('the API token opens masked and empty, and shows only as dots (item 394)', async () => {
    await moveTo('cf-token', 'ArrowDown');
    await key('Enter');
    expect(await oskTitle()).toBe('API token');
    expect(await text('tv-osk-value')).toBe('');
    await typeKeys(WRONG_TOKEN);
    expect(await text('tv-osk-value')).not.toContain(WRONG_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await waitText('tv-cf-token-value', MASK);
    expect(await text('tv-connect')).not.toContain(WRONG_TOKEN);
  });

  it('Library folder opens the folder browser; B steps back to Connect, never past it (items 97, 395)', async () => {
    await moveTo('cf-lib', 'ArrowDown');
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder browser never opened' });
    expect(await $(`${testId('tv-folder-browser')} h1`).getText()).toBe('Library folder');
    expect(await hints()).not.toContain('B Back');
    await key('Escape');
    await $(testId('tv-connect')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'B never came back to Connect' });
    await waitFocused('cf-lib');
    await key('Escape');
    await browser.pause(300);
    expect(await $(testId('tv-connect')).isExisting()).toBe(true);
    // A path can be pasted too.
    await invoke('e2e_clipboard_write', { text: library });
    await key('v');
    await waitText('tv-cf-lib-value', library);
  });

  it('a rejected token comes back to Connect with the reason above the rows (items 398, 515)', async () => {
    expect(await $(testId('tv-connect-go')).getAttribute('aria-disabled')).toBeNull();
    expect(await $(testId('tv-connect-offline')).getAttribute('aria-disabled')).toBeNull();
    expect(await $(`${testId('tv-connect')} .or`).getText()).toBe('or');
    expect(await text('tv-connect-offline')).toBe('Use without a server');
    expect(await $(`${testId('tv-connect')} .note`).getText()).toBe(
      'Without a server, GRID needs only the library folder. Your Library and Backup drives work offline.',
    );
    await press('start');
    await $(testId('tv-connect-reason')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'the rejected token gave no reason' });
    expect((await text('tv-connect-reason')).length).toBeGreaterThan(0);
    expect(await text('tv-connect-reason')).not.toContain(WRONG_TOKEN);
    expect(await $(testId('tv-choose')).isExisting()).toBe(false);
    // The token is asked for again.
    await waitText('tv-cf-token-value', 'Paste or type your token');
  });

  it('Start presses Connect: "Connected to <URL>", then Choose your systems (items 399, 400)', async () => {
    await moveTo('cf-token', 'ArrowUp');
    await key('Enter');
    await typeKeys(FIXTURE_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await press('start');
    await waitToast(/^Connected to http/);
    await $(testId('tv-choose')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'Choose your systems never opened' });
  });

  it('Choose your systems: the intro, a switch row per system, Install and Skip (items 400, 401, 404)', async () => {
    expect(await $(`${testId('tv-choose')} h1`).getText()).toBe('Choose your systems');
    await waitText(
      'tv-choose-intro',
      'Tick the systems you want to play. GRID installs the recommended emulator for each. changes the emulator; skips this step. You can change this later in Settings › Systems.',
    );
    const glyphs = (await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="tv-choose-intro"] [data-glyph]')).map((g) => g.getAttribute('data-glyph')),
    )) as string[];
    expect(glyphs).toEqual(['X', 'MENU']);
    await $(testId('tv-ch-row-ps2')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the systems never loaded' });
    await browser.waitUntil(async () => (await rowKeys('#ch-rows')).includes('ch-windows'), { timeout: TRANSITION_TIMEOUT });
    expect(await rowKeys('#ch-rows')).toEqual(['ch-dc', 'ch-ps2', 'ch-snes', 'ch-zeebo', 'ch-windows']);
    expect(await $$('#ch-rows .sw.on').length).toBe(0);
    const ps2 = await chooseText('ps2');
    expect(ps2[0]).toBe('Sony PlayStation 2');
    expect(ps2[1]).toMatch(/^\d+ games?$/);
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
    await key('Enter');
    await waitToast('No emulator available');
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

  it('Start skips from anywhere; GRID opens on Server › Discover with its first cover focused (items 406, 408, 413, 417)', async () => {
    await moveTo('ch-dc', 'ArrowUp', 8);
    await press('start');
    await waitTab('server');
    expect(await $(testId('tv-choose')).isExisting()).toBe(false);
    // Discover's first cover takes focus; these fixtures may give Discover
    // no covers at all (no dates), and then the Discover row keeps it.
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => {
          const covers = document.querySelectorAll('#disc [data-cover]');
          const f = document.querySelector('[data-tv-root] .f');
          return covers.length ? f === covers[0] : f?.getAttribute('data-k') === 'r-discover';
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
