/**
 * RomM as a storefront (docs/romm-storefront-plan.md Step 8): the shared
 * steps every spec uses to reach RomM.
 *
 * - A fresh data dir starts on the welcome (`welcome`): `connectRomm()`
 *   connects through the welcome's RomM row and presses Continue;
 *   `skipWelcome()` presses Skip for now.
 * - A profile past the welcome starts in the shell: `connectRomm()` connects
 *   through Settings › Game stores' RomM card.
 * - The Library's RomM group: `openRommInstalled()`, `openRommDiscover()`,
 *   `openRommPlatform(slug)` (`library-rail-platform-<slug>`; the platform
 *   screens keep the old Server test ids: `server-platform-header`,
 *   `server-grid`, `game-card-<id>`, `server-search`, …).
 * - TV mode: `tvOpenRommPlatform(slug)` and friends, through Library › RomM.
 */
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from './env.js';
import { dismissToasts } from './toasts.js';

const testId = (id: string) => `[data-testid="${id}"]`;

export type ConnectOptions = {
  /** The server address (default: the stage's mock RomM). */
  url?: string;
  /** The API token (default: the fixture token). */
  token?: string;
  /**
   * What to wait for once connected: `'shell'` (default) waits for the top
   * bar; `'setup'` waits for Choose your systems (the groups with
   * E2E_FIRST_RUN_SETUP); `'none'` returns as soon as RomM shows Connected.
   */
  until?: 'shell' | 'setup' | 'none';
  /** Welcome only: the library folder to type in before connecting. */
  libraryPath?: string;
};

/** Waits for the app to settle on the welcome or the shell; says which. */
export async function startScreen(timeout = APP_START_TIMEOUT): Promise<'welcome' | 'shell' | 'setup'> {
  let found: 'welcome' | 'shell' | 'setup' | null = null;
  await browser.waitUntil(
    async () => {
      if (await $(testId('welcome')).isExisting()) found = 'welcome';
      else if (await $(testId('setup-view')).isExisting()) found = 'setup';
      else if (await $(testId('shell-topbar')).isExisting()) found = 'shell';
      return found !== null;
    },
    { timeout, timeoutMsg: 'neither the welcome nor the shell appeared' },
  );
  return found as unknown as 'welcome' | 'shell' | 'setup';
}

/** Waits until the welcome's library folder is filled in (suggested or typed). */
async function welcomeFolderReady(libraryPath?: string): Promise<void> {
  if (libraryPath !== undefined) {
    await $(testId('welcome-library-path')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('welcome-library-path')).setValue(libraryPath);
  }
  await browser.waitUntil(async () => (await $(testId('welcome-library-path')).getValue()) !== '', {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the welcome never filled in a library folder',
  });
}

async function waitAfterContinue(until: ConnectOptions['until']): Promise<void> {
  if (until === 'setup') {
    await $(testId('setup-view')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'Choose your systems never appeared after Continue',
    });
  } else if (until === 'shell') {
    await $(testId('shell-topbar')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never opened after the welcome',
    });
  }
}

/**
 * A fresh profile: Skip for now on the welcome (the suggested library folder,
 * or `libraryPath`), and the shell opens with nothing connected.
 */
export async function skipWelcome(libraryPath?: string): Promise<void> {
  await $(testId('welcome')).waitForDisplayed({
    timeout: APP_START_TIMEOUT,
    timeoutMsg: 'the welcome never appeared',
  });
  await welcomeFolderReady(libraryPath);
  await $(testId('welcome-skip')).waitForClickable({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'Skip for now never became clickable',
  });
  await $(testId('welcome-skip')).click();
  await $(testId('shell-topbar')).waitForDisplayed({
    timeout: APP_START_TIMEOUT,
    timeoutMsg: 'the shell never opened after Skip for now',
  });
}

/** Fills a RomM sign-in form (`welcome-romm` or `romm-signin`) and submits it. */
async function submitSignIn(base: string, url: string, token: string): Promise<void> {
  await $(testId(`${base}-url`)).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the RomM sign-in form (${base}) never appeared`,
  });
  await $(testId(`${base}-url`)).setValue(url);
  await $(testId(`${base}-secret`)).setValue(token);
  await $(testId(`${base}-submit`)).waitForEnabled({ timeout: TRANSITION_TIMEOUT });
  await $(testId(`${base}-submit`)).click();
}

/** Connects RomM through the welcome's RomM row, then Continue. */
async function connectOnWelcome(url: string, token: string, opts: ConnectOptions): Promise<void> {
  await welcomeFolderReady(opts.libraryPath);
  await $(testId('welcome-connect-romm')).click();
  await submitSignIn('welcome-romm', url, token);
  await $(testId('welcome-row-pill-romm')).waitForDisplayed({
    timeout: APP_START_TIMEOUT,
    timeoutMsg: `RomM's welcome row never showed Connected (error: ${await $(testId('welcome-romm-error'))
      .getText()
      .catch(() => 'none')})`,
  });
  if (opts.until === 'none') return;
  // The "Connected to RomM" card sits over the footer's bottom-right corner.
  await dismissToasts();
  await $(testId('welcome-continue')).waitForClickable({ timeout: TRANSITION_TIMEOUT });
  await $(testId('welcome-continue')).click();
  await waitAfterContinue(opts.until ?? 'shell');
}

/** Opens Settings › Game stores and waits for the RomM card. */
export async function openGameStores(): Promise<void> {
  await $(testId('nav-settings')).click();
  await $(testId('settings-nav-stores')).click();
  await $(testId('store-card-romm')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
}

/** Connects RomM through Settings › Game stores' RomM card. */
async function connectInSettings(url: string, token: string, opts: ConnectOptions): Promise<void> {
  await openGameStores();
  await $(testId('store-card-romm')).scrollIntoView();
  let action = '';
  for (const id of ['connect', 'signin', 'edit']) {
    if (await $(testId(`romm-card-${id}`)).isExisting()) {
      action = id;
      break;
    }
  }
  if (action === '') throw new Error('the RomM card offered neither Connect, Sign in again nor Edit');
  await $(testId(`romm-card-${action}`)).click();
  await $(testId('romm-signin-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  await submitSignIn('romm-signin', url, token);
  if (opts.until === 'setup') {
    await waitAfterContinue('setup');
    return;
  }
  await $(testId('romm-signin-dialog')).waitForExist({
    reverse: true,
    timeout: APP_START_TIMEOUT,
    timeoutMsg: `the RomM sign-in dialog never closed (error: ${await $(testId('romm-signin-error'))
      .getText()
      .catch(() => 'none')})`,
  });
  await browser.waitUntil(async () => (await $(testId('store-card-pill-romm')).getText()).trim() === 'Connected', {
    timeout: APP_START_TIMEOUT,
    timeoutMsg: 'the RomM card never showed Connected',
  });
}

/**
 * Connects RomM to the stage's mock server (or `opts.url`): through the
 * welcome's RomM row on a fresh data dir (then Continue), otherwise through
 * Settings › Game stores. With RomM connected and Open GRID on › Automatic,
 * the shell opens on Library › RomM › Discover after the welcome.
 */
export async function connectRomm(opts: ConnectOptions = {}): Promise<void> {
  const url = opts.url ?? mockUrl();
  const token = opts.token ?? FIXTURE_TOKEN;
  const screen = await startScreen();
  if (screen === 'welcome') await connectOnWelcome(url, token, opts);
  else await connectInSettings(url, token, opts);
}

/** Selects the Library view (the top bar's Library pill). */
export async function openLibrary(): Promise<void> {
  const nav = $(testId('nav-library'));
  await nav.waitForClickable({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the Library pill never appeared' });
  if ((await nav.getAttribute('aria-current')) !== 'page') await nav.click();
  await $(testId('library-rail')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
}

async function clickRail(id: string, what: string): Promise<void> {
  await openLibrary();
  const row = $(testId(id));
  await row.waitForClickable({
    timeout: APP_START_TIMEOUT,
    timeoutMsg: `the Library rail never offered ${what}`,
  });
  await row.click();
}

/** Library › RomM › Installed. */
export async function openRommInstalled(): Promise<void> {
  await clickRail('library-rail-romm-installed', 'RomM › Installed');
  await expect($(testId('library-title'))).toHaveText(expect.stringContaining('Installed'));
}

/** Library › RomM › Discover (RomM connected). */
export async function openRommDiscover(): Promise<void> {
  await clickRail('library-rail-romm-discover', 'RomM › Discover');
  await $(testId('discover')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'RomM › Discover never appeared',
  });
}

/**
 * Library › RomM › <platform> (RomM connected), by the platform's slug
 * (`library-rail-platform-<slug>`), and waits for its header.
 */
export async function openRommPlatform(slug: string): Promise<void> {
  await clickRail(`library-rail-platform-${slug}`, `the RomM platform ${slug}`);
  await $(testId('server-platform-header')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `platform ${slug}'s header never appeared`,
  });
}

/**
 * A mock RomM platform id's slug, from the stage's own `/api/platforms`,
 * so specs written against the old `platform-btn-<id>` ids can open the
 * same platform by id.
 */
export async function platformSlug(id: number): Promise<string> {
  const res = await fetch(`${mockUrl()}/api/platforms`, { headers: { Authorization: `Bearer ${FIXTURE_TOKEN}` } });
  if (!res.ok) throw new Error(`GET /api/platforms answered ${res.status}`);
  const body = (await res.json()) as { id: number; slug: string }[] | { items: { id: number; slug: string }[] };
  const list = Array.isArray(body) ? body : body.items;
  const hit = list.find((p) => p.id === id);
  if (!hit) throw new Error(`the mock RomM has no platform ${id}`);
  return hit.slug;
}
