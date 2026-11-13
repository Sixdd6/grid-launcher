import path from 'node:path';
import { APP_START_TIMEOUT, dataDir } from '../helpers/env.js';
import {
  FAKE_STORE_TOKEN,
  filesContaining,
  filesUnder,
  GOG_REQUEST_LOG,
  gogStatus,
  LOGIN_TIMEOUT,
  openLogin,
  recordStoreEvents,
  storeEvents,
  submitLogin,
  waitLogin,
  waitLoginClosed,
  type StoreLoginEvent,
} from '../helpers/gog.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const ASHEN_TIDE = 'gog:1207664643';

type Entry = { job: string; source_id: string; status: string; error: string };

/**
 * Stage `gog`, secrets (AGENTS.md "Secrets", docs/stores-phase1-plan.md
 * package D): the fake gogdl prints FAKE-STORE-TOKEN-not-real on stdout and
 * stderr for every command, like the real tool. After a sign-in and an
 * install, no log GRID wrote or forwarded, no file in its data folder outside
 * gogdl's own (the token-file exception) and no IPC payload holds it.
 *
 * Needs package A merged (sign-in, install through gogdl).
 */
describe('gog: no store token leaks', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await recordStoreEvents();
  });

  it('signs in', async () => {
    await openLogin();
    await submitLogin('sam');
    await waitLoginClosed();
    const event = await waitLogin(0);
    expect(event.outcome).toBe('connected');
    await browser.waitUntil(async () => (await gogStatus()).tool.state === 'installed', {
      timeout: LOGIN_TIMEOUT,
      timeoutMsg: 'gogdl never showed as installed',
    });
  });

  it('installs a game through gogdl', async () => {
    await invoke('store_install', {
      request: { key: ASHEN_TIDE, platform: 'linux', language: 'en-US', dlc_ids: [], install_dir: '', compat_tool: '' },
    });
    let last: Entry | undefined;
    await browser.waitUntil(
      async () => {
        const entries = (await invoke<{ entries: Entry[] }>('list_downloads')).entries;
        last = entries.find((e) => e.job === 'store' && e.source_id === ASHEN_TIDE);
        return last?.status === 'completed' || last?.status === 'failed';
      },
      { timeout: 60_000, timeoutMsg: 'the GOG install never finished' },
    );
    expect(last?.error).toBe('');
    expect(last?.status).toBe('completed');
    expect((await gogStatus()).installed_count).toBe(1);
  });

  it('syncs its cloud saves', async () => {
    const state = await invoke<{ state: string }>('store_cloud_sync', { key: ASHEN_TIDE });
    expect(['synced', 'none', 'off']).toContain(state.state);
  });

  it('the token reached gogdl’s own file and nowhere else', async () => {
    const data = dataDir();
    const toolDir = path.join('stores', 'gogdl');
    // The flow really carried the token (else this spec proves nothing).
    expect(filesContaining(filesUnder(path.join(data, toolDir)), FAKE_STORE_TOKEN).length).toBeGreaterThan(0);

    const dataFiles = filesUnder(data, [toolDir]);
    expect(filesContaining(dataFiles, FAKE_STORE_TOKEN)).toEqual([]);

    // The app's stdout and stderr (tracing), as the tauri service captured
    // them, the wdio logs (they hold every IPC answer) and the mock's log.
    const logDir = process.env.E2E_WDIO_LOG_DIR;
    const logs = [...(logDir ? filesUnder(logDir) : []), GOG_REQUEST_LOG];
    expect(logs.length).toBeGreaterThan(1);
    expect(filesContaining(logs, FAKE_STORE_TOKEN)).toEqual([]);

    // The events the frontend got.
    const events = JSON.stringify([
      await storeEvents<StoreLoginEvent>('store-login'),
      await storeEvents('store-notice'),
      await storeEvents('stores-changed'),
    ]);
    expect(events).not.toContain(FAKE_STORE_TOKEN);
  });
});
