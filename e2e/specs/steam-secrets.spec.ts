import { readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir } from '../helpers/env.js';
import { filesContaining, filesUnder, recordStoreEvents, storeEvents } from '../helpers/gogLogin.js';
import {
  FAKE_ACCOUNT_NAME,
  GAMES,
  invoke,
  MAIN_WINDOW,
  SAM,
  STEAM_KEY,
  STEAM_REQUEST_LOG,
  STEAM_REVOKED_KEY,
  steamRequests,
  steamSave,
  steamStatus,
  steamTest,
  waitSteam,
  waitSteamArgv,
} from '../helpers/steamSetup.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const COMET = `steam:${GAMES.quietComet}`;
const FERRY = `steam:${GAMES.moonlitFerry}`;
/** Everything the app answered over IPC in this spec, kept for the final scan. */
const answers: unknown[] = [];

type Entry = { id: number; job: string; kind: string; source_id: string; status: string; detail: string; error: string };
type Snapshot = { sessions: { key: string }[] };

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await invoke<T>(cmd, args);
  answers.push(answer);
  return answer;
}

/** Records `sessions-changed` in the main window (`window.__steamSessions`). */
async function recordSessions(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await browser.execute(async () => {
    const w = window as unknown as {
      __steamSessions?: unknown[];
      __TAURI_INTERNALS__: {
        transformCallback: (cb: (e: { payload: unknown }) => void) => number;
        invoke: (c: string, a: unknown) => Promise<unknown>;
      };
    };
    if (w.__steamSessions) return;
    w.__steamSessions = [];
    const handler = w.__TAURI_INTERNALS__.transformCallback((e) => w.__steamSessions!.push(e.payload));
    await w.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'sessions-changed', target: { kind: 'Any' }, handler });
  });
}

async function sessions(): Promise<Snapshot[]> {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await browser.execute(() => (window as unknown as { __steamSessions?: unknown[] }).__steamSessions ?? [])) as Snapshot[];
}

async function steamEntry(key: string): Promise<Entry | undefined> {
  const entries = (await call<{ entries: Entry[] }>('list_downloads')).entries;
  return entries.filter((e) => e.source_id === key).pop();
}

/**
 * Stage `steam`, secrets (AGENTS.md "Secrets", docs/stores-phase4-plan.md
 * "E2E plan"), with debug prints on (e2e.sh runs this spec with
 * RUST_LOG=debug). The fake key FAKE-STEAM-KEY-not-real goes to the app only
 * through `steam_save` / `steam_test` (reversed on the WebDriver wire, see
 * helpers/steamSetup.ts), and the fake steam prints it on stdout and
 * stderr. After Connect, Test, Refresh, an install, a launch, an uninstall
 * and Disconnect it appears in no GRID log line, no `stores-changed` /
 * `store-notice` / `sessions-changed` event, no `invoke` answer, not in the
 * config, the registry, `stores/cache/*`, the Downloads snapshot or the
 * page. The mock's log shows the key arrived only as the `key` query
 * parameter of GetOwnedGames / GetPlayerSummaries, never on GetItems or
 * art, and never with a Referer or a Cookie. The Steam login name
 * (FAKE-ACCOUNT-NAME-not-shown in loginusers.vdf and registry.vdf) appears
 * nowhere either.
 *
 * Needs package A (the client, the Web API, the manifests, the opener and
 * the alive check).
 */
describe('steam: the Web API key never leaks', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
    await recordSessions();
  });

  it('tests a revoked key and the good one, then connects', async () => {
    answers.push(await steamTest(SAM, STEAM_REVOKED_KEY));
    answers.push(await steamTest(SAM, STEAM_KEY));
    expect(await steamSave(SAM, STEAM_KEY)).toEqual({ kind: 'ok', owned_count: 4 });
    const status = await waitSteam((s) => s.installed_count === 3, 'the installed games never showed');
    expect(status.steam.key).toBe('ok');
    answers.push(status);
  });

  it('refreshes the owned list', async () => {
    const before = (await steamRequests()).length;
    await call('store_refresh', { source: 'steam' });
    await browser.waitUntil(async () => (await steamRequests()).slice(before).some((r) => r.path.includes('GetOwnedGames')), {
      timeout: 20_000,
      timeoutMsg: 'the refresh never asked Steam',
    });
    await waitSteam((s) => s.owned === 'ready', 'the refresh never ended');
    answers.push(await call('store_library', { source: 'steam' }));
  });

  it('installs Quiet Comet through the fake steam', async () => {
    await call('steam_open', { key: COMET, action: 'install' });
    await waitSteamArgv([`steam://install/${GAMES.quietComet}`]);
    await browser.waitUntil(async () => (await steamEntry(COMET))?.status === 'completed', {
      timeout: 40_000,
      timeoutMsg: 'the Steam install never completed',
    });
    expect((await steamEntry(COMET))!.detail).toBe('Installed by Steam');
  });

  it('launches Moonlit Ferry through Steam and sees the session end', async () => {
    await call('launch_game_key', { key: FERRY });
    await waitSteamArgv(['-silent', `steam://rungameid/${GAMES.moonlitFerry}`]);
    await browser.waitUntil(async () => (await sessions()).some((s) => s.sessions.some((x) => x.key === FERRY)), {
      timeout: 20_000,
      timeoutMsg: 'the watched session never started',
    });
    await browser.waitUntil(
      async () => {
        const all = await sessions();
        return all.length > 0 && all[all.length - 1].sessions.every((x) => x.key !== FERRY);
      },
      { timeout: 30_000, timeoutMsg: 'the watched session never ended' },
    );
  });

  it('uninstalls Quiet Comet in Steam; it leaves the Library', async () => {
    await call('steam_open', { key: COMET, action: 'uninstall' });
    await waitSteamArgv([`steam://uninstall/${GAMES.quietComet}`]);
    await waitSteam((s) => s.installed_count === 3, 'Quiet Comet never left', 40_000);
    await call('steam_open', { key: FERRY, action: 'validate' });
    await waitSteamArgv([`steam://validate/${GAMES.moonlitFerry}`]);
  });

  it('nothing GRID wrote, logged, emitted or answered holds the key or the login name', async () => {
    const data = dataDir();
    // GRID's own files: config, registry, caches, sessions; not the fake
    // Steam folder (Steam's files hold the login name by design).
    const files = filesUnder(data, ['steam']);
    expect(files.some((f) => f.endsWith('grid-launcher.db'))).toBe(true);
    expect(files.some((f) => f.includes(path.join('stores', 'cache', 'steam-owned.json')))).toBe(true);
    expect(filesContaining(files, STEAM_KEY)).toEqual([]);
    expect(filesContaining(files, FAKE_ACCOUNT_NAME)).toEqual([]);
    expect(readFileSync(configPath(), 'utf8')).toContain('[stores.steam]');

    // The app's stdout and stderr (tracing at debug), as the tauri service
    // captured them, and the wdio logs (they hold every IPC answer).
    const logDir = process.env.E2E_WDIO_LOG_DIR;
    const logs = logDir ? filesUnder(logDir) : [];
    expect(logs.length).toBeGreaterThan(0);
    expect(filesContaining(logs, 'DEBUG').length).toBeGreaterThan(0);
    expect(filesContaining(logs, STEAM_KEY)).toEqual([]);
    expect(filesContaining(logs, FAKE_ACCOUNT_NAME)).toEqual([]);
    expect(filesContaining([STEAM_REQUEST_LOG], 'FAKE-STEAM-KEY')).toEqual([]);

    // The events the frontend got, the answers, the page.
    const page = await browser.execute(() => [
      document.body.innerText,
      ...Array.from(document.querySelectorAll('input, textarea')).map((el) => (el as HTMLInputElement).value),
    ]);
    const seen = JSON.stringify([
      await storeEvents('store-notice'),
      await storeEvents('stores-changed'),
      await sessions(),
      await call('stores_status'),
      await call('store_library', { source: 'steam' }),
      await call('list_downloads'),
      answers,
      page,
    ]);
    expect(seen.includes(STEAM_KEY)).toBe(false);
    expect(seen.includes(FAKE_ACCOUNT_NAME)).toBe(false);

    // The key went only to the Web API's key calls, as the `key` parameter.
    const requests = await steamRequests();
    const keyed = requests.filter((r) => r.key !== null);
    expect(keyed.length).toBeGreaterThan(0);
    expect(keyed.every((r) => r.path.includes('GetOwnedGames') || r.path.includes('GetPlayerSummaries'))).toBe(true);
    expect(requests.filter((r) => r.path.includes('GetItems') || r.path.startsWith('/art/')).every((r) => r.key === null)).toBe(true);
    expect(requests.some((r) => r.path.includes('GetItems'))).toBe(true);
    expect(requests.filter((r) => r.referer || r.cookie || r.refused)).toEqual([]);
  });

  it('Disconnect deletes the key and the caches; the logs still hold no key', async () => {
    await call('store_disconnect', { source: 'steam' });
    const status = await steamStatus();
    expect(status.connection).toBe('not_connected');
    const files = filesUnder(dataDir(), ['steam']);
    expect(files.filter((f) => f.includes(path.join('stores', 'cache', 'steam-')))).toEqual([]);
    expect(filesContaining(files, STEAM_KEY)).toEqual([]);
    await expect(steamTest(SAM, null)).rejects.toThrow(/Paste your key first/);
    const logDir = process.env.E2E_WDIO_LOG_DIR;
    expect(filesContaining(logDir ? filesUnder(logDir) : [], STEAM_KEY)).toEqual([]);
  });
});
