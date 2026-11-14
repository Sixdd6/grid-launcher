import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir } from '../helpers/env.js';
import {
  EPIC_REQUEST_LOG,
  epicStatus,
  FAKE_STORE_TOKEN,
  filesContaining,
  filesUnder,
  LOGIN_TIMEOUT,
  MAIN_WINDOW,
  openEpicLogin,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitEpic,
  waitLogin,
  waitLoginClosed,
  type StoreLoginEvent,
} from '../helpers/epicLogin.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const STARLIGHT = 'epic:Starlight';

type Entry = { job: string; source_id: string; status: string; error: string };
type Snapshot = { warning: string | null };

/** The game's argv file the seed's umu-run / wine stand-ins write. */
const argvFile = () => process.env.E2E_EPIC_ARGV_FILE ?? path.join(dataDir(), 'epic-game-argv.log');

/**
 * Records `sessions-changed` in the main window (`window.__epicSessions`),
 * as `recordStoreEvents` does for the store events.
 */
async function recordSessions(): Promise<void> {
  await browser.switchToWindow(MAIN_WINDOW);
  await browser.execute(async () => {
    const w = window as unknown as {
      __epicSessions?: unknown[];
      __TAURI_INTERNALS__: {
        transformCallback: (cb: (e: { payload: unknown }) => void) => number;
        invoke: (c: string, a: unknown) => Promise<unknown>;
      };
    };
    if (w.__epicSessions) return;
    w.__epicSessions = [];
    const handler = w.__TAURI_INTERNALS__.transformCallback((e) => w.__epicSessions!.push(e.payload));
    await w.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'sessions-changed', target: { kind: 'Any' }, handler });
  });
}

async function sessions(): Promise<Snapshot[]> {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await browser.execute(() => (window as unknown as { __epicSessions?: unknown[] }).__epicSessions ?? [])) as Snapshot[];
}

/**
 * Stage `epic`, secrets (AGENTS.md "Secrets", docs/stores-phase2-plan.md
 * "E2E plan"): the fake legendary prints FAKE-STORE-TOKEN-not-real on
 * stdout and stderr for every command, keeps it in `user.json`, and hands
 * it out as the launch ticket's exchange code. After a sign-in, an install,
 * a launch and a cloud sync, no log GRID wrote or forwarded, no IPC payload
 * (the recorded events and the `invoke` answers in the wdio logs), no
 * session record and no file in its data folder outside legendary's own
 * (the store-token exception) holds it. The game's argv does hold it (the
 * exchange code reached the game), and the early-exit warning shows
 * `<redacted>` in its place.
 *
 * Needs packages A (sign-in, install) and B (the launch ticket, cloud
 * saves) merged.
 */
describe('epic: no store token leaks', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
    await recordSessions();
  });

  it('signs in', async () => {
    await openEpicLogin();
    await submitLogin('sam');
    await waitLoginClosed();
    const event = await waitLogin(0);
    expect(event.error).toBe('');
    expect(event.outcome).toBe('connected');
    await waitEpic((s) => s.tool.state === 'installed' && s.owned === 'ready', 'legendary and the owned list never showed as ready');
  });

  it('installs a game through legendary', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('store_install', {
      request: { key: STARLIGHT, platform: 'windows', language: '', dlc_ids: [], install_dir: '', compat_tool: 'GE-Proton9-20' },
    });
    let last: Entry | undefined;
    await browser.waitUntil(
      async () => {
        const entries = (await invoke<{ entries: Entry[] }>('list_downloads')).entries;
        last = entries.find((e) => e.job === 'store' && e.source_id === STARLIGHT);
        return last?.status === 'completed' || last?.status === 'failed';
      },
      { timeout: 60_000, timeoutMsg: 'the Epic install never finished' },
    );
    expect(last?.error).toBe('');
    expect(last?.status).toBe('completed');
    expect((await epicStatus()).installed_count).toBe(1);
  });

  it('launches it with the launch ticket; the early-exit warning hides the code', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    await invoke('launch_game_key', { key: STARLIGHT });
    let warning = '';
    await browser.waitUntil(
      async () => {
        warning = (await sessions()).map((s) => s.warning ?? '').find((w) => w.includes('exited immediately')) ?? '';
        return warning !== '';
      },
      { timeout: LOGIN_TIMEOUT, timeoutMsg: 'the game never exited early' },
    );
    expect(warning).toContain('<redacted>');
    expect(warning).not.toContain(FAKE_STORE_TOKEN);
    // The exchange code reached the game, last on its command line.
    const argv = readFileSync(argvFile(), 'utf8').split('\n');
    expect(argv).toContain(`-AUTH_PASSWORD=${FAKE_STORE_TOKEN}`);
    expect(argv.some((a) => a.endsWith('Starlight.exe'))).toBe(true);
  });

  it('syncs its cloud saves', async () => {
    await browser.switchToWindow(MAIN_WINDOW);
    const state = await invoke<{ state: string }>('store_cloud_sync', { key: STARLIGHT });
    expect(['synced', 'none', 'off']).toContain(state.state);
  });

  it('the token reached legendary’s own files and the game, and nowhere else', async () => {
    const data = dataDir();
    const toolDir = path.join('stores', 'legendary');
    // The flow really carried the token (else this spec proves nothing).
    const toolFiles = filesUnder(path.join(data, toolDir));
    expect(filesContaining(toolFiles, FAKE_STORE_TOKEN).length).toBeGreaterThan(0);
    // legendary writes 0644; GRID makes its tree private after every run.
    for (const file of toolFiles) {
      expect({ file, mode: statSync(file).mode & 0o077 }).toEqual({ file, mode: 0 });
    }

    const dataFiles = filesUnder(data, [toolDir, path.relative(data, argvFile())]);
    expect(filesContaining(dataFiles, FAKE_STORE_TOKEN)).toEqual([]);

    // The app's stdout and stderr (tracing), as the tauri service captured
    // them, the wdio logs (they hold every IPC answer) and the mock's log.
    const logDir = process.env.E2E_WDIO_LOG_DIR;
    const logs = [...(logDir ? filesUnder(logDir) : []), EPIC_REQUEST_LOG];
    expect(logs.length).toBeGreaterThan(1);
    expect(filesContaining(logs, FAKE_STORE_TOKEN)).toEqual([]);

    // The events the frontend got, and the sessions (their records).
    const events = JSON.stringify([
      await storeEvents<StoreLoginEvent>('store-login'),
      await storeEvents('store-notice'),
      await storeEvents('stores-changed'),
      await sessions(),
      await invoke('stores_status'),
      await invoke('store_library', { source: 'epic' }),
    ]);
    expect(events).not.toContain(FAKE_STORE_TOKEN);
  });
});
