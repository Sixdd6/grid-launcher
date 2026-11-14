/**
 * Game stores (Amazon Games) helpers for the login and secrets specs:
 * Amazon's status, the login window opened for Amazon, the mock Amazon's
 * request log and state, nile's folder. The store-event recording, the
 * login window's form and the file scans are the GOG helpers'
 * (`gogLogin.ts`): they are store-neutral.
 *
 * Phase 1 lesson: when the login window opens, WebDriver may follow it as
 * its active window. Every helper here switches to the window it needs
 * first, and the specs switch back to the main window before they invoke a
 * command or read an event.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { dataDir } from './env.js';
import { LOGIN_TIMEOUT, LOGIN_WINDOW, loginWindowOpen, MAIN_WINDOW, type StoreStatus } from './gogLogin.js';
import { invoke } from './tv.js';

export {
  clearStoreEvents,
  FAKE_STORE_TOKEN,
  filesContaining,
  filesUnder,
  LOGIN_TIMEOUT,
  LOGIN_WINDOW,
  loginWindowOpen,
  MAIN_WINDOW,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitLogin,
  waitLoginClosed,
  waitLoginForm,
  type StoreLoginEvent,
  type StoreNotice,
  type StoreStatus,
} from './gogLogin.js';

/** What the mock Amazon's sign-in page hands out (a fixture, not a secret). */
export const AMAZON_GOOD_CODE = 'FAKE-E2E-AMAZON-CODE';
/** The account the fake nile signs in (nile's `Username`). */
export const AMAZON_USERNAME = 'sam.plays';
/** The fixture's product ids (e2e/fixtures-amazon/library.json). */
export const FOGLINE = 'amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a1';
export const KETTLE_DRUM = 'amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a2';
export const LOWSUN = 'amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a3';

const here = path.dirname(fileURLToPath(import.meta.url));
/** mock-amazon.mjs's request log (one JSON line per request). */
export const AMAZON_REQUEST_LOG = path.join(here, '..', 'last-run-amazon-requests.log');

export const amazonUrl = (): string => {
  const value = process.env.E2E_AMAZON_URL;
  if (!value) throw new Error('E2E_AMAZON_URL is not set: run the amazon group through scripts/e2e.sh');
  return value;
};

/** GRID's nile folder (NILE_CONFIG_PATH) and nile's own files inside it. */
export const nileToolDir = (): string => path.join(dataDir(), 'stores', 'nile');
export const nileConfigDir = (): string => path.join(nileToolDir(), 'nile');

export type AmazonStatus = StoreStatus & {
  in_rail: boolean;
  cloud_saves: boolean;
  tool: { state: string; error: string; name: string; version: string; license: string; source_url: string; size_bytes: number };
};

export const amazonStatus = async (): Promise<AmazonStatus> => {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await invoke<AmazonStatus[]>('stores_status')).find((s) => s.source === 'amazon')!;
};

/**
 * `store_connect` for Amazon (GRID installs nile when it is missing and
 * asks it for this attempt's login URL first), then waits for the login
 * window and switches to it.
 */
export async function openAmazonLogin(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await invoke('store_connect', { source: 'amazon' });
  await browser.waitUntil(loginWindowOpen, { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the login window never opened' });
  await browser.switchToWindow(LOGIN_WINDOW);
}

/** `store_cancel_login` for Amazon from the main window, then waits for the close. */
export async function cancelAmazonLogin(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await invoke('store_cancel_login', { source: 'amazon' });
  await browser.waitUntil(async () => !(await loginWindowOpen()), {
    timeout: LOGIN_TIMEOUT,
    timeoutMsg: 'the login window never closed',
  });
  await browser.switchToWindow(MAIN_WINDOW);
}

/** The mock Amazon's request log lines. */
export function amazonRequests(): { method: string; path: string }[] {
  try {
    return readFileSync(AMAZON_REQUEST_LOG, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch {
    return [];
  }
}

export async function amazonControl(route: string): Promise<void> {
  await fetch(`${amazonUrl()}/__e2e__/${route}`, { method: 'POST' });
}

/** How often the mock served its finish (must stay 0) and its plain home page. */
export async function amazonMockState(): Promise<{ finishLoads: number; homeLoads: number }> {
  return (await (await fetch(`${amazonUrl()}/__e2e__/state`)).json()) as { finishLoads: number; homeLoads: number };
}

/** Waits until a predicate on Amazon's status holds. */
export async function waitAmazon(
  test: (s: AmazonStatus) => boolean,
  timeoutMsg: string,
  timeout = LOGIN_TIMEOUT,
): Promise<AmazonStatus> {
  let last: AmazonStatus | undefined;
  await browser.waitUntil(
    async () => {
      last = await amazonStatus();
      return test(last);
    },
    { timeout, timeoutMsg },
  );
  return last!;
}
