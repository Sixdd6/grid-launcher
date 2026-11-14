/**
 * Game stores (Epic) helpers for the login and secrets specs: Epic's
 * status, the login window opened for Epic, the mock Epic's request log and
 * state. The store-event recording, the login window's form and the file
 * scans are the GOG helpers' (`gogLogin.ts`): they are store-neutral.
 *
 * Phase 1 lesson: when the login window opens, WebDriver may follow it as
 * its active window. Every helper here switches to the window it needs
 * first, and the specs switch back to the main window before they invoke a
 * command or read an event.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
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

/** What the mock Epic's sign-in page hands out (a fixture, not a secret). */
export const EPIC_GOOD_CODE = 'FAKE-E2E-EPIC-CODE';
/** The user agent Epic needs to end the sign-in with the launcher redirect. */
export const EPIC_USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) EpicGamesLauncher';
/** The finish path the login window must refuse. */
export const EPIC_FINISH_PATH = '/launcher/authorized';

const here = path.dirname(fileURLToPath(import.meta.url));
/** mock-epic.mjs's request log (one JSON line per request). */
export const EPIC_REQUEST_LOG = path.join(here, '..', 'last-run-epic-requests.log');

export const epicUrl = (): string => {
  const value = process.env.E2E_EPIC_URL;
  if (!value) throw new Error('E2E_EPIC_URL is not set: run the epic group through scripts/e2e.sh');
  return value;
};

export type EpicStatus = StoreStatus & { in_rail: boolean; tool: { state: string; error: string; name: string; version: string } };

export const epicStatus = async (): Promise<EpicStatus> => {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await invoke<EpicStatus[]>('stores_status')).find((s) => s.source === 'epic')!;
};

/** `store_connect` for Epic, then waits for the login window and switches to it. */
export async function openEpicLogin(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await invoke('store_connect', { source: 'epic' });
  await browser.waitUntil(loginWindowOpen, { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the login window never opened' });
  await browser.switchToWindow(LOGIN_WINDOW);
}

/** `store_cancel_login` for Epic from the main window, then waits for the close. */
export async function cancelEpicLogin(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await invoke('store_cancel_login', { source: 'epic' });
  await browser.waitUntil(async () => !(await loginWindowOpen()), {
    timeout: LOGIN_TIMEOUT,
    timeoutMsg: 'the login window never closed',
  });
  await browser.switchToWindow(MAIN_WINDOW);
}

/** The mock Epic's request log lines. */
export function epicRequests(): { method: string; path: string }[] {
  try {
    return readFileSync(EPIC_REQUEST_LOG, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch {
    return [];
  }
}

export async function epicControl(route: string): Promise<void> {
  await fetch(`${epicUrl()}/__e2e__/${route}`, { method: 'POST' });
}

/** How often the mock served its finish page (must stay 0) and the pop-up's page. */
export async function epicMockState(): Promise<{ authorizedLoads: number; popupLoads: number }> {
  return (await (await fetch(`${epicUrl()}/__e2e__/state`)).json()) as { authorizedLoads: number; popupLoads: number };
}

/** Waits until a predicate on Epic's status holds. */
export async function waitEpic(test: (s: EpicStatus) => boolean, timeoutMsg: string, timeout = LOGIN_TIMEOUT): Promise<EpicStatus> {
  let last: EpicStatus | undefined;
  await browser.waitUntil(
    async () => {
      last = await epicStatus();
      return test(last);
    },
    { timeout, timeoutMsg },
  );
  return last!;
}
