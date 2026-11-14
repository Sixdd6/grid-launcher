import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir } from '../helpers/env.js';
import {
  AMAZON_GOOD_CODE,
  AMAZON_REQUEST_LOG,
  amazonStatus,
  FAKE_STORE_TOKEN,
  filesContaining,
  filesUnder,
  FOGLINE,
  LOGIN_TIMEOUT,
  MAIN_WINDOW,
  nileConfigDir,
  nileToolDir,
  openAmazonLogin,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitAmazon,
  waitLogin,
  waitLoginClosed,
  type StoreLoginEvent,
} from '../helpers/amazonLogin.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const KEY = `amazon:${FOGLINE}`;
/** The FUEL variable that carries the user's given name. */
const DISPLAY_NAME_VAR = 'AMAZON_GAMES_FUEL_DISPLAY_NAME';

type Entry = { id: number; job: string; kind: string; source_id: string; status: string; error: string };
type Snapshot = { warning: string | null };

/** The game's argv and env files the seed's umu-run / wine stand-ins write. */
const argvFile = () => process.env.E2E_AMAZON_ARGV_FILE ?? path.join(dataDir(), 'amazon-game-argv.log');
const envFile = () => process.env.E2E_AMAZON_ENV_FILE ?? path.join(dataDir(), 'amazon-game-env.log');

/**
 * Records `sessions-changed` in the main window (`window.__amazonSessions`),
 * as `recordStoreEvents` does for the store events.
 */
async function recordSessions(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await browser.execute(async () => {
    const w = window as unknown as {
      __amazonSessions?: unknown[];
      __TAURI_INTERNALS__: {
        transformCallback: (cb: (e: { payload: unknown }) => void) => number;
        invoke: (c: string, a: unknown) => Promise<unknown>;
      };
    };
    if (w.__amazonSessions) return;
    w.__amazonSessions = [];
    const handler = w.__TAURI_INTERNALS__.transformCallback((e) => w.__amazonSessions!.push(e.payload));
    await w.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'sessions-changed', target: { kind: 'Any' }, handler });
  });
}

async function sessions(): Promise<Snapshot[]> {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await browser.execute(() => (window as unknown as { __amazonSessions?: unknown[] }).__amazonSessions ?? [])) as Snapshot[];
}

/** Waits for the newest Downloads entry of Fogline's `kind` job to end; answers it. */
async function waitJob(kind: string, after: number): Promise<Entry> {
  let last: Entry | undefined;
  await browser.waitUntil(
    async () => {
      const entries = (await invoke<{ entries: Entry[] }>('list_downloads')).entries;
      last = entries.filter((e) => e.job === 'store' && e.source_id === KEY && e.kind === kind && e.id > after).pop();
      return last?.status === 'completed' || last?.status === 'failed';
    },
    { timeout: 60_000, timeoutMsg: `the Amazon ${kind} never finished` },
  );
  return last!;
}

async function lastEntryId(): Promise<number> {
  const entries = (await invoke<{ entries: Entry[] }>('list_downloads')).entries;
  return entries.reduce((max, e) => Math.max(max, e.id), 0);
}

/** Every folder under `dir`, `dir` itself included. */
function foldersUnder(dir: string): string[] {
  const out = [dir];
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) out.push(...foldersUnder(full));
  }
  return out;
}

/**
 * Stage `amazon`, secrets (AGENTS.md "Secrets", docs/stores-phase3-plan.md
 * "E2E plan"): the fake nile prints FAKE-STORE-TOKEN-not-real on stdout and
 * stderr for every command, keeps it in its token file and hands it out as
 * the login's code_verifier. After a sign-in, an install, an update, a
 * verify and a launch, no log GRID wrote or forwarded, no IPC payload (the
 * recorded events and the `invoke` answers in the wdio logs), no session
 * record and no file in its data folder outside nile's own (the
 * store-token exception) holds it or the login code; nile's folder is
 * owner-only although nile writes 0644. The FUEL variables reached the game
 * and no log names the display-name variable. Disconnect removes nile's
 * folder even when `auth --logout` fails.
 *
 * Needs package A (sign-in, install, update, verify, launch prep,
 * disconnect) merged.
 */
describe('amazon: no store token leaks', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
    await recordSessions();
  });

  it('signs in', async () => {
    await openAmazonLogin();
    await submitLogin('sam');
    await waitLoginClosed();
    const event = await waitLogin(0);
    expect(event.error).toBe('');
    expect(event.outcome).toBe('connected');
    await waitAmazon((s) => s.tool.state === 'installed' && s.owned === 'ready', 'nile and the owned list never showed as ready');
  });

  it('installs Fogline through nile', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    const after = await lastEntryId();
    await invoke('store_install', {
      request: { key: KEY, platform: 'windows', language: '', dlc_ids: [], install_dir: '', compat_tool: 'GE-Proton9-20' },
    });
    const entry = await waitJob('store_install', after);
    expect(entry.error).toBe('');
    expect(entry.status).toBe('completed');
    expect((await amazonStatus()).installed_count).toBe(1);
  });

  it('updates it when nile has an update, and verifies it', async () => {
    writeFileSync(path.join(nileConfigDir(), '.fake-update'), '');
    await browser.switchToWindow(MAIN_WINDOW);
    let after = await lastEntryId();
    await invoke('store_update', { key: KEY });
    const update = await waitJob('store_update', after);
    expect(update.error).toBe('');
    expect(update.status).toBe('completed');
    const installed = JSON.parse(readFileSync(path.join(nileConfigDir(), 'installed.json'), 'utf8')) as { version: string }[];
    expect(installed[0].version).toBe('e2e-fogline-version-2');

    after = await lastEntryId();
    await invoke('store_repair', { key: KEY });
    const verify = await waitJob('store_repair', after);
    expect(verify.error).toBe('');
    expect(verify.status).toBe('completed');
  });

  it('launches it with nile’s FUEL environment; the game gets it, nothing logs it', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('launch_game_key', { key: KEY });
    let warning = '';
    await browser.waitUntil(
      async () => {
        warning = (await sessions()).map((s) => s.warning ?? '').find((w) => w.includes('exited immediately')) ?? '';
        return warning !== '';
      },
      { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the game never exited early' },
    );
    expect(warning).not.toContain(FAKE_STORE_TOKEN);
    expect(warning).not.toContain(DISPLAY_NAME_VAR);
    const env = readFileSync(envFile(), 'utf8').split('\n');
    expect(env).toContain('AMAZON_GAMES_FUEL_PRODUCT_SKU=FAKE-SKU-FOGLINE');
    expect(env).toContain('AMAZON_GAMES_FUEL_ENTITLEMENT_ID=0d3e5b2a-7c41-4f0e-9a11-a1f0911e0001');
    expect(env).toContain(`${DISPLAY_NAME_VAR}=sam.plays`);
    expect(env.some((line) => line.startsWith('FUEL_DIR=') && line.endsWith(path.join('SDK', 'Amazon Games Services', 'Legacy')))).toBe(true);
    expect(env.some((line) => line.startsWith('AMAZON_GAMES_SDK_PATH='))).toBe(true);
    // fuel.json's argument reached the game once (no doubling with the row's).
    const argv = readFileSync(argvFile(), 'utf8').split('\n');
    expect(argv.some((a) => a.endsWith('Fogline.exe'))).toBe(true);
    expect(argv.filter((a) => a === '-fuel-e2e').length).toBe(1);
  });

  it('the token reached nile’s own files only, and nile’s folder is owner-only', async () => {
    const data = dataDir();
    const toolDir = path.relative(data, nileToolDir());
    // The flow really carried the token (else this spec proves nothing).
    const toolFiles = filesUnder(nileToolDir());
    expect(filesContaining(toolFiles, FAKE_STORE_TOKEN).length).toBeGreaterThan(0);
    // nile writes 0644 and 0755; GRID makes its tree private around every run.
    for (const file of toolFiles) {
      expect({ file, mode: statSync(file).mode & 0o077 }).toEqual({ file, mode: 0 });
    }
    for (const dir of foldersUnder(nileToolDir())) {
      expect({ dir, mode: statSync(dir).mode & 0o077 }).toEqual({ dir, mode: 0 });
    }

    const dataFiles = filesUnder(data, [toolDir]);
    expect(filesContaining(dataFiles, FAKE_STORE_TOKEN)).toEqual([]);
    expect(filesContaining(filesUnder(data), AMAZON_GOOD_CODE)).toEqual([]);

    // The app's stdout and stderr (tracing), as the tauri service captured
    // them, the wdio logs (they hold every IPC answer) and the mock's log.
    const logDir = process.env.E2E_WDIO_LOG_DIR;
    const logs = [...(logDir ? filesUnder(logDir) : []), AMAZON_REQUEST_LOG];
    expect(logs.length).toBeGreaterThan(1);
    expect(filesContaining(logs, FAKE_STORE_TOKEN)).toEqual([]);
    expect(filesContaining(logs, AMAZON_GOOD_CODE)).toEqual([]);
    expect(filesContaining(logs, DISPLAY_NAME_VAR)).toEqual([]);

    // The events the frontend got, and the sessions (their records).
    const events = JSON.stringify([
      await storeEvents<StoreLoginEvent>('store-login'),
      await storeEvents('store-notice'),
      await storeEvents('stores-changed'),
      await sessions(),
      await invoke('stores_status'),
      await invoke('store_library', { source: 'amazon' }),
    ]);
    expect(events).not.toContain(FAKE_STORE_TOKEN);
    expect(events).not.toContain(AMAZON_GOOD_CODE);
    expect(events).not.toContain(DISPLAY_NAME_VAR);
  });

  it('Disconnect deletes nile’s folder even when its logout fails; the game stays', async () => {
    writeFileSync(path.join(nileConfigDir(), '.fake-logout-fail'), '');
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_disconnect', { source: 'amazon' });
    expect(existsSync(nileToolDir())).toBe(false);
    const status = await amazonStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.installed_count).toBe(1);
    expect(filesContaining(filesUnder(dataDir()), FAKE_STORE_TOKEN)).toEqual([]);
  });
});
