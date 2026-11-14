import { APP_START_TIMEOUT } from '../helpers/env.js';
import {
  cancelEpicLogin,
  clearStoreEvents,
  EPIC_FINISH_PATH,
  EPIC_GOOD_CODE,
  EPIC_USER_AGENT,
  epicControl,
  epicMockState,
  epicRequests,
  epicStatus,
  LOGIN_TIMEOUT,
  LOGIN_WINDOW,
  loginWindowOpen,
  MAIN_WINDOW,
  openEpicLogin,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitEpic,
  waitLogin,
  waitLoginClosed,
  waitLoginForm,
  type StoreLoginEvent,
  type StoreNotice,
} from '../helpers/epicLogin.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

async function loginCount(): Promise<number> {
  return (await storeEvents<StoreLoginEvent>('store-login')).length;
}

async function epicNotices(): Promise<string[]> {
  return (await storeEvents<StoreNotice>('store-notice')).filter((n) => n.source === 'epic').map((n) => n.message);
}

/**
 * Stage `epic`, the login window for Epic Games (docs/stores-phase2-plan.md
 * package C; GameStores "Login window"), driven through `invoke` and the
 * login window itself, on a profile used without a server. The mock Epic
 * (mock-romm/mock-epic.mjs) serves the sign-in page and the finish
 * redirect; the fake legendary (fake-tools/legendary.mjs) redeems the code.
 *
 * Cancel, ✕, the network error, the user agent, the localhost rule, the
 * pop-up and a denied sign-in need no grid-core Epic code. The refused
 * code, the sign-in, the owned list and Disconnect need package A
 * (`EpicService::complete_login`, `account`, `owned`, `disconnect`).
 */
describe('epic: the login window', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
  });

  afterEach(async () => {
    if (await loginWindowOpen()) await cancelEpicLogin();
    await browser.switchToWindow(MAIN_WINDOW);
    await clearStoreEvents();
  });

  it('starts not connected and outside the rail', async () => {
    const status = await epicStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.username).toBe('');
    expect(status.in_rail).toBe(false);
    expect(status.tool.name).toBe('legendary');
    expect(status.tool.version).toBe('0.21.1');
  });

  it('opens Epic’s sign-in page with the Epic Games Launcher’s user agent', async () => {
    await openEpicLogin();
    await waitLoginForm();
    expect(await browser.execute(() => navigator.userAgent)).toBe(EPIC_USER_AGENT);
    await cancelEpicLogin();
  });

  it('Cancel closes the window silently', async () => {
    const before = await loginCount();
    await openEpicLogin();
    await waitLoginForm();
    await cancelEpicLogin();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('cancelled');
    expect(event.source).toBe('epic');
    expect(await epicNotices()).toEqual([]);
    expect((await epicStatus()).connection).toBe('not_connected');
  });

  it('closing the window (✕) is a cancel too', async () => {
    const before = await loginCount();
    await openEpicLogin();
    await waitLoginForm();
    await browser.closeWindow();
    await waitLoginClosed();
    expect((await waitLogin(before)).outcome).toBe('cancelled');
  });

  it('a network error stays inside the window and Try again reloads the page', async () => {
    await epicControl('login-down');
    try {
      await openEpicLogin();
      await browser.waitUntil(
        async () => ((await browser.execute(() => document.body?.innerText ?? '')) as string).includes('Couldn’t reach'),
        { timeout: LOGIN_TIMEOUT, timeoutMsg: "GRID's network error page never showed" },
      );
      const text = (await browser.execute(() => document.body.innerText)) as string;
      expect(text).toContain('Check your connection; the page reloads when you try again.');
      expect(await loginWindowOpen()).toBe(true);
    } finally {
      await epicControl('login-up');
    }
    await browser.switchToWindow(LOGIN_WINDOW);
    await browser.execute(() => (document.querySelector('#retry') as HTMLButtonElement).click());
    await waitLoginForm();
  });

  it('refuses a navigation to anything else on this computer, silently', async () => {
    const before = await loginCount();
    await openEpicLogin();
    // The mock's sign-in answers user "local" with a 302 to
    // http://localhost:<its port + 1>/.
    await submitLogin('local');
    await browser.pause(1500);
    expect(await loginWindowOpen()).toBe(true);
    await browser.switchToWindow(LOGIN_WINDOW);
    const href = (await browser.execute(() => location.href)) as string;
    expect(href).not.toContain('localhost');
    expect(await loginCount()).toBe(before);
    expect((await epicStatus()).connection).toBe('not_connected');
  });

  it('a pop-up (a third-party sign-in) opens in the same login window', async () => {
    await openEpicLogin();
    await waitLoginForm();
    await browser.execute(() => (document.querySelector('#popup') as HTMLAnchorElement).click());
    await browser.waitUntil(async () => (await browser.execute(() => Boolean(document.querySelector('#open-popup')))) as boolean, {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'the pop-up page never loaded',
    });
    await browser.execute(() => (document.querySelector('#open-popup') as HTMLButtonElement).click());
    await browser.waitUntil(async () => (await epicMockState()).popupLoads > 0, {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'the pop-up never loaded in the login window',
    });
    const handles = await browser.getWindowHandles();
    expect(handles.filter((h) => h !== MAIN_WINDOW)).toEqual([LOGIN_WINDOW]);
    await waitLoginForm();
  });

  it('a redirect without a code fails, with its error on the store', async () => {
    const before = await loginCount();
    await openEpicLogin();
    await submitLogin('denied');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('failed');
    expect(event.error).toBe('Epic Games returned an error. Try again.');
    // The sign-in outcome card is the frontend's, from `store-login`.
    expect(await epicNotices()).toEqual([]);
    const status = await epicStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.last_error).toContain('Epic Games returned an error');
  });

  // Needs package A (`complete_login`: legendary auth, the code on stdin).
  it('a code legendary refuses fails the sign-in', async () => {
    const before = await loginCount();
    await openEpicLogin();
    await submitLogin('refused');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('failed');
    expect(event.username).toBe('');
    expect(event.error).not.toBe('not implemented');
    expect((await epicStatus()).connection).toBe('not_connected');
  });

  // Needs package A (`complete_login`, `owned`).
  it('signs in: the redirect is caught in Rust, the window closes, Epic connects', async () => {
    const before = await loginCount();
    await openEpicLogin();
    await submitLogin('sam');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.error).toBe('');
    expect(event.outcome).toBe('connected');
    expect(event.username).toBe('SamPlays');
    // The finish redirect never loads: GRID refuses the navigation.
    expect((await epicMockState()).authorizedLoads).toBe(0);
    expect(epicRequests().filter((r) => r.path === EPIC_FINISH_PATH)).toEqual([]);
    const status = await waitEpic((s) => s.owned === 'ready', 'the owned list never loaded');
    expect(status.connection).toBe('connected');
    expect(status.username).toBe('SamPlays');
    expect(status.tool.state).toBe('installed');
    expect(status.owned_count).toBe(6);
    expect(status.in_rail).toBe(true);
    expect(status.last_error).toBe('');
    const messages = await epicNotices();
    // "Connected to Epic Games as …" is the frontend's card (from `store-login`).
    expect(messages).not.toContain('Connected to Epic Games as SamPlays');
    // Iron Orchard and Northern Static are Not supported: 6 found, 4 to install.
    expect(messages).toContain('Epic Games: 6 games found · 4 to install');
    // No IPC payload carries the code.
    const payloads = JSON.stringify(await storeEvents('store-login')) + JSON.stringify(await storeEvents('store-notice'));
    expect(payloads).not.toContain(EPIC_GOOD_CODE);
  });

  it('lists the owned games: Not supported names the launcher', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    const library = await invoke<{ installed: unknown[]; available: { store_id: string; not_supported: string; platforms: string[]; cover: string }[] }>(
      'store_library',
      { source: 'epic' },
    );
    expect(library.installed).toEqual([]);
    expect(library.available.map((g) => g.store_id)).toEqual(['Starlight', 'Vector', 'Glimmer', 'IronOrchard', 'NorthernStatic', 'Rustbelt']);
    const notSupported = Object.fromEntries(library.available.map((g) => [g.store_id, g.not_supported]));
    expect(notSupported.IronOrchard).toBe('the EA app');
    expect(notSupported.NorthernStatic).toBe('Ubisoft Connect');
    expect(notSupported.Glimmer).toBe('');
    expect(library.available.every((g) => g.platforms.join() === 'windows')).toBe(true);
    expect(library.available[0].cover.startsWith(process.env.E2E_EPIC_URL ?? '')).toBe(true);
  });

  it('Install legendary installs it again on request', async () => {
    await clearStoreEvents();
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_install_tool', { source: 'epic' });
    await browser.waitUntil(async () => (await epicNotices()).includes('legendary installed'), {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'legendary never said it was installed',
    });
    expect((await epicStatus()).tool.state).toBe('installed');
  });

  // Needs package A (`disconnect`: auth --delete, then the tool folder).
  it('Disconnect forgets the login', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_disconnect', { source: 'epic' });
    const status = await epicStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.username).toBe('');
    expect(status.owned_count).toBe(0);
    expect(status.in_rail).toBe(false);
  });
});
