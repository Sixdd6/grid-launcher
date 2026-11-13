import { APP_START_TIMEOUT } from '../helpers/env.js';
import {
  clearStoreEvents,
  gogControl,
  gogRequests,
  gogStatus,
  LOGIN_TIMEOUT,
  LOGIN_WINDOW,
  loginWindowOpen,
  MAIN_WINDOW,
  openLogin,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitLogin,
  waitLoginClosed,
  waitLoginForm,
  type StoreLoginEvent,
  type StoreNotice,
} from '../helpers/gogLogin.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

async function loginCount(): Promise<number> {
  return (await storeEvents<StoreLoginEvent>('store-login')).length;
}

/**
 * Stage `gog`, the login window (docs/stores-phase1-plan.md package D;
 * GameStores card "Login window", TVStores "Signing in"), driven through
 * `invoke` and the login window itself: no store UI exists yet. A profile
 * used without a server; the mock GOG serves the login page.
 *
 * With package A merged every case runs end to end: the successful
 * sign-in's `complete_login` runs the fake gogdl's `auth --code`.
 */
describe('gog: the login window', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
  });

  afterEach(async () => {
    if (await loginWindowOpen()) {
      await browser.switchToWindow(MAIN_WINDOW);
      await invoke('store_cancel_login', { source: 'gog' });
      await waitLoginClosed();
    }
    await browser.switchToWindow(MAIN_WINDOW);
    await clearStoreEvents();
  });

  it('starts not connected and outside the rail', async () => {
    const status = await gogStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.username).toBe('');
  });

  it('Cancel closes the window silently', async () => {
    const before = await loginCount();
    await openLogin();
    await waitLoginForm();
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_cancel_login', { source: 'gog' });
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('cancelled');
    expect(event.source).toBe('gog');
    expect(await storeEvents<StoreNotice>('store-notice')).toEqual([]);
    expect((await gogStatus()).connection).toBe('not_connected');
  });

  it('closing the window (✕) is a cancel too', async () => {
    const before = await loginCount();
    await openLogin();
    await waitLoginForm();
    await browser.closeWindow();
    await waitLoginClosed();
    expect((await waitLogin(before)).outcome).toBe('cancelled');
  });

  it('a second Connect focuses the open window', async () => {
    await openLogin();
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_connect', { source: 'gog' });
    const handles = await browser.getWindowHandles();
    expect(handles.filter((h) => h === LOGIN_WINDOW).length).toBe(1);
  });

  it('a network error stays inside the window and Try again reloads the page', async () => {
    await gogControl('auth-down');
    try {
      await openLogin();
      await browser.waitUntil(
        async () => ((await browser.execute(() => document.body?.innerText ?? '')) as string).includes('Couldn’t reach'),
        { timeout: LOGIN_TIMEOUT, timeoutMsg: "GRID's network error page never showed" },
      );
      const text = (await browser.execute(() => document.body.innerText)) as string;
      expect(text).toContain('Check your connection; the page reloads when you try again.');
      expect(await loginWindowOpen()).toBe(true);
    } finally {
      await gogControl('auth-up');
    }
    await browser.switchToWindow(LOGIN_WINDOW);
    await browser.execute(() => (document.querySelector('#retry') as HTMLButtonElement).click());
    await waitLoginForm();
  });

  it('TV mode types into the page and moves between its fields', async () => {
    await openLogin();
    await waitLoginForm();
    // Focus lands on the first field when the page loads.
    await browser.waitUntil(async () => (await browser.execute(() => document.activeElement?.id)) === 'username', {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'focus never landed on the first field',
    });
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_login_type', { text: 'sam"<x>' });
    await invoke('store_login_type', { text: '\t' });
    await invoke('store_login_type', { text: 'pw1' });
    await invoke('store_login_type', { text: '\u0008' });
    await browser.switchToWindow(LOGIN_WINDOW);
    const fields = (await browser.execute(() => ({
      user: (document.querySelector('#username') as HTMLInputElement).value,
      pass: (document.querySelector('#password') as HTMLInputElement).value,
      focused: document.activeElement?.id,
    }))) as { user: string; pass: string; focused: string };
    expect(fields).toEqual({ user: 'sam"<x>', pass: 'pw', focused: 'password' });
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_login_type', { text: '\u001b[Z' });
    await browser.switchToWindow(LOGIN_WINDOW);
    expect(await browser.execute(() => document.activeElement?.id)).toBe('username');
  });

  it('typing with no login window open is refused', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    let error = '';
    try {
      await invoke('store_login_type', { text: 'x' });
    } catch (e) {
      error = String(e);
    }
    expect(error).toContain("The sign-in window isn't open.");
  });

  it('a redirect without a code fails, with its error on the store', async () => {
    const before = await loginCount();
    await openLogin();
    await submitLogin('denied');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('failed');
    expect(event.error).toBe('GOG returned an error. Try again.');
    // The sign-in outcome card is the frontend's, from `store-login`.
    expect(await storeEvents<StoreNotice>('store-notice')).toEqual([]);
    const status = await gogStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.last_error).toContain('GOG returned an error');
  });

  it('a code gogdl refuses fails the sign-in', async () => {
    const before = await loginCount();
    await openLogin();
    await submitLogin('refused');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.outcome).toBe('failed');
    expect(event.username).toBe('');
    expect((await gogStatus()).connection).toBe('not_connected');
  });

  // Needs package A (GogService::complete_login and account).
  it('signs in: the redirect is caught in Rust, the window closes, GOG connects', async () => {
    const before = await loginCount();
    await openLogin();
    await submitLogin('sam');
    await waitLoginClosed();
    const event = await waitLogin(before);
    expect(event.error).toBe('');
    expect(event.outcome).toBe('connected');
    expect(event.username).toBe('sam_plays');
    // The finish redirect never loads: GRID refuses the navigation.
    expect(gogRequests().filter((r) => r.path === '/on_login_success')).toEqual([]);
    await browser.waitUntil(async () => (await gogStatus()).owned === 'ready', {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'the owned list never loaded',
    });
    const status = await gogStatus();
    expect(status.connection).toBe('connected');
    expect(status.username).toBe('sam_plays');
    expect(status.owned_count).toBe(3);
    expect(status.last_error).toBe('');
    const messages = (await storeEvents<StoreNotice>('store-notice')).map((n) => n.message);
    // "Connected to GOG as …" is the frontend's card (from `store-login`).
    expect(messages).not.toContain('Connected to GOG as sam_plays');
    expect(messages).toContain('GOG: 3 games found · 3 to install');
    // No IPC payload carries the code.
    const payloads = JSON.stringify(await storeEvents('store-login')) + JSON.stringify(await storeEvents('store-notice'));
    expect(payloads).not.toContain('FAKE-E2E-GOG-CODE');
  });

  it('Disconnect forgets the login and wipes the login window data', async () => {
    await invoke('store_disconnect', { source: 'gog' });
    const status = await gogStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.username).toBe('');
  });
});
