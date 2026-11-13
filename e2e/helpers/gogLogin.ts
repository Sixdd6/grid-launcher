/**
 * Game stores (GOG) helpers: the store commands through `invoke`, the
 * backend's store events recorded in the main window, and the login window
 * (a second webview window, label `store-login`, driven with WebDriver's
 * window switching). No store UI is needed.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { invoke } from './tv.js';

export const LOGIN_WINDOW = 'store-login';
export const MAIN_WINDOW = 'main';
export const FAKE_STORE_TOKEN = 'FAKE-STORE-TOKEN-not-real';
/** What the mock login page's code is (a fixture, not a secret). */
export const GOOD_CODE = 'FAKE-E2E-GOG-CODE';
export const LOGIN_TIMEOUT = 20_000;

const here = path.dirname(fileURLToPath(import.meta.url));
/** mock-gog.mjs's request log (one JSON line per request). */
export const GOG_REQUEST_LOG = path.join(here, '..', 'last-run-gog-requests.log');

export const gogUrl = (): string => {
  const value = process.env.E2E_GOG_URL;
  if (!value) throw new Error('E2E_GOG_URL is not set: run the gog group through scripts/e2e.sh');
  return value;
};

export type StoreStatus = {
  source: string;
  connection: 'connected' | 'expired' | 'not_connected';
  username: string;
  tool: { state: string; error: string };
  owned: string;
  owned_count: number;
  installed_count: number;
  last_error: string;
};
export type StoreLoginEvent = { source: string; outcome: string; username: string; error: string };
export type StoreNotice = { source: string | null; message: string; error: boolean; action: { kind: string } | null };

export const gogStatus = async (): Promise<StoreStatus> =>
  (await invoke<StoreStatus[]>('stores_status')).find((s) => s.source === 'gog')!;

/**
 * Records `store-login`, `store-notice` and `stores-changed` in the main
 * window (`window.__gogEvents`), through the event plugin the main window's
 * capability already grants.
 */
export async function recordStoreEvents(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await browser.execute(async () => {
    const w = window as unknown as {
      __gogEvents?: Record<string, unknown[]>;
      __TAURI_INTERNALS__: {
        transformCallback: (cb: (e: { payload: unknown }) => void) => number;
        invoke: (c: string, a: unknown) => Promise<unknown>;
      };
    };
    if (w.__gogEvents) return;
    w.__gogEvents = { 'store-login': [], 'store-notice': [], 'stores-changed': [] };
    for (const event of Object.keys(w.__gogEvents)) {
      const handler = w.__TAURI_INTERNALS__.transformCallback((e) => w.__gogEvents![event].push(e.payload));
      await w.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event, target: { kind: 'Any' }, handler });
    }
  });
}

export async function storeEvents<T>(name: 'store-login' | 'store-notice' | 'stores-changed'): Promise<T[]> {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await browser.execute(
    (n: string) => ((window as unknown as { __gogEvents: Record<string, unknown[]> }).__gogEvents ?? {})[n] ?? [],
    name,
  )) as T[];
}

export async function clearStoreEvents(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await browser.execute(() => {
    const w = window as unknown as { __gogEvents?: Record<string, unknown[]> };
    for (const key of Object.keys(w.__gogEvents ?? {})) w.__gogEvents![key] = [];
  });
}

/** Waits for the next `store-login` event after `before` of them. */
export async function waitLogin(before: number, timeout = LOGIN_TIMEOUT): Promise<StoreLoginEvent> {
  let found: StoreLoginEvent | undefined;
  await browser.waitUntil(
    async () => {
      const all = await storeEvents<StoreLoginEvent>('store-login');
      found = all[before];
      return Boolean(found);
    },
    { timeout, timeoutMsg: 'no store-login event arrived' },
  );
  return found!;
}

export const loginWindowOpen = async (): Promise<boolean> =>
  (await browser.getWindowHandles()).includes(LOGIN_WINDOW);

/** `store_connect`, then waits for the login window and switches to it. */
export async function openLogin(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await invoke('store_connect', { source: 'gog' });
  await browser.waitUntil(loginWindowOpen, { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the login window never opened' });
  await browser.switchToWindow(LOGIN_WINDOW);
}

/** Waits for the mock's login form inside the login window. */
export async function waitLoginForm(): Promise<void> {
  await browser.switchToWindow(LOGIN_WINDOW);
  await browser.waitUntil(
    async () => (await browser.execute(() => Boolean(document.querySelector('#username')))) as boolean,
    { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the mock login page never loaded' },
  );
}

/** Fills the mock login form in the login window and submits it. */
export async function submitLogin(username: string): Promise<void> {
  await waitLoginForm();
  await browser.execute((u: string) => {
    (document.querySelector('#username') as HTMLInputElement).value = u;
    (document.querySelector('#password') as HTMLInputElement).value = 'pw';
    (document.querySelector('#login') as HTMLButtonElement).click();
  }, username);
}

export async function waitLoginClosed(): Promise<void> {
  await browser.waitUntil(async () => !(await loginWindowOpen()), {
    timeout: LOGIN_TIMEOUT,
    timeoutMsg: 'the login window never closed',
  });
  await browser.switchToWindow(MAIN_WINDOW);
}

/** The mock GOG's request log lines. */
export function gogRequests(): { method: string; path: string; auth: boolean }[] {
  try {
    return readFileSync(GOG_REQUEST_LOG, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch {
    return [];
  }
}

export async function gogControl(route: string): Promise<void> {
  await fetch(`${gogUrl()}/__e2e__/${route}`, { method: 'POST' });
}

/** Every file under `dir`, skipping folders `skip` names (relative). */
export function filesUnder(dir: string, skip: string[] = []): string[] {
  const out: string[] = [];
  const walk = (current: string) => {
    let names: string[];
    try {
      names = readdirSync(current);
    } catch {
      return;
    }
    for (const name of names) {
      const full = path.join(current, name);
      if (skip.some((s) => full === path.join(dir, s))) continue;
      let info;
      try {
        info = statSync(full);
      } catch {
        continue;
      }
      if (info.isDirectory()) walk(full);
      else if (info.isFile() && info.size < 64 * 1024 * 1024) out.push(full);
    }
  };
  walk(dir);
  return out;
}

/** The files among `files` whose bytes hold `needle`. */
export function filesContaining(files: string[], needle: string): string[] {
  const bytes = Buffer.from(needle, 'utf8');
  return files.filter((file) => {
    try {
      return readFileSync(file).includes(bytes);
    } catch {
      return false;
    }
  });
}
