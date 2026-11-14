import { existsSync } from 'node:fs';
import { APP_START_TIMEOUT } from '../helpers/env.js';
import {
  AMAZON_GOOD_CODE,
  AMAZON_USERNAME,
  amazonControl,
  amazonMockState,
  amazonStatus,
  amazonUrl,
  cancelAmazonLogin,
  clearStoreEvents,
  FOGLINE,
  KETTLE_DRUM,
  LOGIN_TIMEOUT,
  LOGIN_WINDOW,
  loginWindowOpen,
  LOWSUN,
  MAIN_WINDOW,
  nileToolDir,
  openAmazonLogin,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitAmazon,
  waitLogin,
  waitLoginClosed,
  waitLoginForm,
  type StoreLoginEvent,
  type StoreNotice,
} from '../helpers/amazonLogin.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

async function loginCount(): Promise<number> {
  return (await storeEvents<StoreLoginEvent>('store-login')).length;
}

async function amazonNotices(): Promise<string[]> {
  return (await storeEvents<StoreNotice>('store-notice')).filter((n) => n.source === 'amazon').map((n) => n.message);
}

async function loginHref(): Promise<string> {
  await browser.switchToWindow(LOGIN_WINDOW);
  return (await browser.execute(() => location.href)) as string;
}

/**
 * Stage `amazon`, the login window for Amazon Games
 * (docs/stores-phase3-plan.md package C; GameStores "Login window"), driven
 * through `invoke` and the login window itself, on a profile used without a
 * server. Before the window opens, GRID asks nile for this attempt's login
 * URL (`auth --login --non-interactive`); the mock Amazon
 * (mock-romm/mock-amazon.mjs) serves that sign-in page and the finish
 * redirect, and the fake nile (fake-tools/nile.mjs) registers with the code.
 *
 * In e2e the fake nile is always installed (GRID_LAUNCHER_E2E_TOOL_NILE), so
 * "nile installs before the window" is the unit tests'
 * (`stores_service::amazon`); here the window opening on nile's URL proves
 * nile ran first.
 *
 * The login URL, Cancel, ✕, the network error, Amazon's home page and a
 * denied sign-in need package A's `start_login`. The refused code, the
 * sign-in, the owned list and Disconnect need its `complete_login`, `sync`
 * and `disconnect`.
 */
describe('amazon: the login window', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
  });

  afterEach(async () => {
    if (await loginWindowOpen()) await cancelAmazonLogin();
    await browser.switchToWindow(MAIN_WINDOW);
    await clearStoreEvents();
  });

  it('starts not connected and outside the rail, with nile 1.2.0 and no cloud saves', async () => {
    const status = await amazonStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.username).toBe('');
    expect(status.in_rail).toBe(false);
    expect(status.cloud_saves).toBe(false);
    expect(status.tool.name).toBe('nile');
    expect(status.tool.version).toBe('1.2.0');
    expect(status.tool.license).toBe('GPL-3.0');
    expect(status.tool.source_url).toBe('https://github.com/imLinguin/nile');
  });

  it('opens the sign-in page nile printed for this attempt', async () => {
    await openAmazonLogin();
    await waitLoginForm();
    const href = new URL(await loginHref());
    expect(href.origin).toBe(new URL(amazonUrl()).origin);
    expect(href.pathname).toBe('/ap/signin');
    expect(href.searchParams.get('openid.assoc_handle')).toBe('amzn_sonic_games_launcher');
    expect(href.searchParams.get('openid.oa2.client_id')?.startsWith('device:')).toBe(true);
    await cancelAmazonLogin();
  });

  it('Cancel closes the window silently', async () => {
    const before = await loginCount();
    await openAmazonLogin();
    await waitLoginForm();
    await cancelAmazonLogin();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('cancelled');
    expect(event.source).toBe('amazon');
    expect(await amazonNotices()).toEqual([]);
    expect((await amazonStatus()).connection).toBe('not_connected');
  });

  it('closing the window (✕) is a cancel too', async () => {
    const before = await loginCount();
    await openAmazonLogin();
    await waitLoginForm();
    await browser.closeWindow();
    await waitLoginClosed();
    expect((await waitLogin(before)).outcome).toBe('cancelled');
  });

  it('a network error stays inside the window and Try again reloads the page', async () => {
    await amazonControl('login-down');
    try {
      await openAmazonLogin();
      await browser.waitUntil(
        async () => ((await browser.execute(() => document.body?.innerText ?? '')) as string).includes('Couldn’t reach'),
        { timeout: LOGIN_TIMEOUT, timeoutMsg: "GRID's network error page never showed" },
      );
      const text = (await browser.execute(() => document.body.innerText)) as string;
      expect(text).toContain('Check your connection; the page reloads when you try again.');
      expect(await loginWindowOpen()).toBe(true);
    } finally {
      await amazonControl('login-up');
    }
    await browser.switchToWindow(LOGIN_WINDOW);
    await browser.execute(() => (document.querySelector('#retry') as HTMLButtonElement).click());
    await waitLoginForm();
  });

  it('Amazon’s home page (the finish’s path without its assoc_handle) loads as an ordinary page', async () => {
    const before = await loginCount();
    const homeLoads = (await amazonMockState()).homeLoads;
    await openAmazonLogin();
    await waitLoginForm();
    await browser.execute(() => (document.querySelector('#home') as HTMLAnchorElement).click());
    await browser.waitUntil(async () => (await browser.execute(() => Boolean(document.querySelector('#mock-amazon-home')))) as boolean, {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: "Amazon's home page never loaded in the login window",
    });
    expect((await amazonMockState()).homeLoads).toBeGreaterThan(homeLoads);
    expect(await loginWindowOpen()).toBe(true);
    expect(await loginCount()).toBe(before);
    expect((await amazonStatus()).connection).toBe('not_connected');
  });

  it('a redirect without a code fails, with its error on the store', async () => {
    const before = await loginCount();
    await openAmazonLogin();
    await submitLogin('denied');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('failed');
    expect(event.error).toBe('Amazon Games returned an error. Try again.');
    // The sign-in outcome card is the frontend's, from `store-login`.
    expect(await amazonNotices()).toEqual([]);
    const status = await amazonStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.last_error).toContain('Amazon Games returned an error');
    expect((await amazonMockState()).finishLoads).toBe(0);
  });

  // Needs package A (`complete_login`: nile register, the code on argv).
  it('a code nile refuses fails the sign-in', async () => {
    const before = await loginCount();
    await openAmazonLogin();
    await submitLogin('refused');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('failed');
    expect(event.username).toBe('');
    expect(event.error).not.toBe('not implemented');
    expect(event.error).not.toContain('FAKE-');
    expect((await amazonStatus()).connection).toBe('not_connected');
  });

  // Needs package A (`complete_login`, `sync`).
  it('signs in: the finish is caught in Rust, the window closes, Amazon connects and syncs', async () => {
    const before = await loginCount();
    await openAmazonLogin();
    await submitLogin('sam');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.error).toBe('');
    expect(event.outcome).toBe('connected');
    expect(event.username).toBe(AMAZON_USERNAME);
    // The finish redirect never loads: GRID refuses the navigation.
    expect((await amazonMockState()).finishLoads).toBe(0);
    const status = await waitAmazon((s) => s.owned === 'ready', 'the owned list never loaded');
    expect(status.connection).toBe('connected');
    expect(status.username).toBe(AMAZON_USERNAME);
    expect(status.tool.state).toBe('installed');
    // Fogline's second entitlement is the same product: 3 games, not 4.
    expect(status.owned_count).toBe(3);
    expect(status.in_rail).toBe(true);
    expect(status.last_error).toBe('');
    const messages = await amazonNotices();
    // "Connected to Amazon Games as …" is the frontend's card (from `store-login`).
    expect(messages).not.toContain(`Connected to Amazon Games as ${AMAZON_USERNAME}`);
    expect(messages).toContain('Amazon Games: 3 games found · 3 to install');
    // No IPC payload carries the code.
    const payloads = JSON.stringify(await storeEvents('store-login')) + JSON.stringify(await storeEvents('store-notice'));
    expect(payloads).not.toContain(AMAZON_GOOD_CODE);
  });

  it('lists the owned games from nile’s library: Windows only, no cloud saves, art at the mock', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    const library = await invoke<{
      installed: unknown[];
      available: { store_id: string; title: string; platforms: string[]; cover: string; cloud: string; dlc: unknown[]; origin: string | null }[];
    }>('store_library', { source: 'amazon' });
    expect(library.installed).toEqual([]);
    expect(library.available.map((g) => g.store_id)).toEqual([FOGLINE, KETTLE_DRUM, LOWSUN]);
    expect(library.available.map((g) => g.title)).toEqual(['Fogline', 'Kettle Drum', 'Lowsun']);
    expect(library.available.every((g) => g.platforms.join() === 'windows' && g.cloud === 'none' && g.dlc.length === 0)).toBe(true);
    expect(library.available[0].cover.startsWith(amazonUrl())).toBe(true);
  });

  it('Install nile installs it again on request', async () => {
    await clearStoreEvents();
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_install_tool', { source: 'amazon' });
    await browser.waitUntil(async () => (await amazonNotices()).includes('nile installed'), {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'nile never said it was installed',
    });
    expect((await amazonStatus()).tool.state).toBe('installed');
  });

  // Needs package A (`disconnect`: auth --logout, then the tool folder).
  it('Disconnect forgets the login and deletes nile’s folder', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_disconnect', { source: 'amazon' });
    const status = await amazonStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.username).toBe('');
    expect(status.owned_count).toBe(0);
    expect(status.in_rail).toBe(false);
    expect(existsSync(nileToolDir())).toBe(false);
  });
});
