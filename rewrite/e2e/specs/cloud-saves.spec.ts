import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  REAP_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `cloud-saves`: manual/auto upload, auto-download-on-launch,
 * retention pruning, and the native-platform block reason — the milestone 6
 * exit gate (task-20-brief.md).
 *
 * NOT covered here: xemu's raw-disk save sync. No xemu binary is available
 * in CI, so that flow has no E2E stub to drive; its coverage is the Task 16
 * wiremock integration test (`cloud_service.rs`'s xemu tests) instead.
 *
 * Pre-seeded by `e2e/seed/cloud-saves-seed.mjs` (Ruling A, task-7-brief.md)
 * against `e2e/fixtures-cloud-saves/{platforms,roms,rom-details,saves}.json`:
 * three installed games on "Super Nintendo Entertainment System" sharing
 * one configured emulator, "TestEmu", and one save directory —
 *
 * - rom 601 "SaveSyncManual": a local save file already on disk, no server
 *   records. Scenario 1 (manual upload).
 * - rom 602 "SaveSyncLaunch": NO local save file; the mock's
 *   `saves.json` seeds one server record for it. Scenarios 2 and 3
 *   (auto-download-on-launch, auto-upload-on-exit). Its "TestEmu" stub
 *   (`play-stub.sh`) writes fresh save content only once it receives
 *   SIGTERM (the Details "Stop" button) — see the seed script's own doc
 *   comment for why that ordering is what lets this spec observe the
 *   download completing strictly before the emulator's own writes.
 * - rom 603 "SaveSyncRetention": a local save file, plus FOUR server
 *   records in one slot group (`saves.json`). Scenario 4 (retention
 *   pruning against the default `cloud_save_retention_limit` of 3).
 *
 * The mock RomM server (`e2e/mock-romm/server.mjs`) runs as its own
 * process (scripts/e2e.sh's `run_group_attempt`), so this spec inspects
 * what it received through its live `GET /__e2e__/requests` introspection
 * endpoint (added for this group) rather than an in-process request log.
 */
describe('cloud-saves', () => {
  const savesDir = () => path.join(dataDir(), 'cloud-saves');
  const launchSaveFile = () => path.join(savesDir(), 'savesynclaunch.sav');
  const playArgvFile = () => path.join(dataDir(), 'stubs', 'play-stub.args');

  /** Every request the mock has received so far, newest last. */
  async function mockRequests(): Promise<
    Array<{
      method: string;
      path: string;
      query?: Record<string, string>;
      multipart?: Array<{ name: string; filename?: string; text: string }>;
      bodyJson?: unknown;
    }>
  > {
    const res = await fetch(`${mockUrl()}/__e2e__/requests`);
    return res.json();
  }

  async function waitForConfigLine(line: string) {
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(configPath(), 'utf-8').includes(line);
        } catch {
          return false;
        }
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `config.toml never got: ${line}` },
    );
  }

  async function openDetails(romId: number) {
    await $(testId('platform-btn-1')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId(`game-card-${romId}`)).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId(`game-card-${romId}`)).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the details overlay never opened for rom ${romId}`,
    });
  }

  async function closeDetails() {
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the details overlay never closed',
    });
  }

  async function openSavePanel() {
    // The cloud toggles live on the Saves tab (design §7). The tab itself
    // is always mounted; the toggle only exists once the tab is showing.
    await $(testId('details-tab-saves')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details popup never rendered its Saves tab',
    });
    await $(testId('details-tab-saves')).click();
    await $(testId('details-cloud-save-toggle')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the "Manage Saves" toggle never appeared',
    });
    await $(testId('details-cloud-save-toggle')).click();
    await $(testId('cloud-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the cloud save panel never opened',
    });
  }

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after connecting',
    });

    // Auto-download-on-launch and auto-upload-on-exit both default to
    // `true` (config.rs's `default_true`), so the seed's bare config.toml
    // already has them on. Only the upload delay (default 3s) needs
    // changing — down to 0 so scenario 3 doesn't need a real wait.
    await $(testId('nav-settings')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the settings view never rendered',
    });
    await $(testId('settings-nav-cloud-saves')).click();
    await $(testId('settings-page-cloud-saves')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Settings › Cloud saves never rendered',
    });
    const delayInput = $(testId('cloud-settings-upload-delay'));
    await delayInput.waitForExist({ timeout: TRANSITION_TIMEOUT });
    await delayInput.clearValue();
    await delayInput.setValue('0');
    await $(testId('cloud-settings-save')).click();
    await waitForConfigLine('auto_cloud_save_upload_delay_seconds = 0');
    await $(testId('nav-server')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the settings view never went away',
    });
  });

  it('manual upload: the panel Upload button POSTs one overwrite=true multipart request', async () => {
    await openDetails(601);
    await openSavePanel();

    const before = (await mockRequests()).length;
    await $(testId('cloud-upload')).click();

    // Round 4: every upload now reports through the shell toast, so a user
    // who has scrolled the panel away still learns the result. rom 601 has
    // exactly one local save file (cloud-saves-seed.mjs), so the text is the
    // Info branch of `upload_completion_message` (transfer.rs:897-900).
    await $(testId('toast')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared after the manual upload',
    });
    await expect($(testId('toast'))).toHaveText('SaveSyncManual — Uploaded 1 save files.');

    await browser.waitUntil(
      async () => (await mockRequests()).length > before,
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the mock never received the manual upload' },
    );

    const uploads = (await mockRequests())
      .slice(before)
      .filter((r) => r.method === 'POST' && r.path.startsWith('/api/saves?'));
    expect(uploads).toHaveLength(1);
    expect(uploads[0].query?.rom_id).toBe('601');
    expect(uploads[0].query?.overwrite).toBe('true');
    expect(uploads[0].multipart).toHaveLength(1);
    expect(uploads[0].multipart?.[0].filename).toBe('savesyncmanual.sav');
    expect(uploads[0].multipart?.[0].text).toBe('local-save-for-manual-upload');

    await closeDetails();
  });

  it('launch: the seeded cloud save is restored to disk before the stub emulator runs', async () => {
    expect(existsSync(launchSaveFile())).toBe(false);

    await openDetails(602);
    await $(testId('details-play')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-play never appeared for the pre-seeded installed game',
    });
    await $(testId('details-play')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-playing-chip never appeared after Play',
    });

    // `launch_game` (commands.rs) awaits `auto_restore_before_launch`
    // BEFORE spawning the emulator, so the stub's own argv file appearing
    // is proof the restore already finished — reading the save file's
    // content right after confirms it actually landed on disk, not just
    // that the ordering was structurally guaranteed.
    await browser.waitUntil(() => existsSync(playArgvFile()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the play-stub never wrote its argv file',
    });
    expect(readFileSync(launchSaveFile(), 'utf-8')).toBe('cloud-downloaded-save-bytes');
  });

  it('exit: the auto upload fires after the (zeroed) delay with the stub-written content', async () => {
    const UPLOAD_AFTER_EXIT_TIMEOUT = 12_000;
    const before = (await mockRequests()).length;

    await $(testId('details-stop')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: REAP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'details-playing-chip never cleared after Stop within the reaper window',
    });

    // The stub's SIGTERM trap (play-stub.sh, cloud-saves-seed.mjs)
    // overwrites the save file with fresh "gameplay" content right before
    // it exits — this is what the auto-upload below must send, not the
    // stale cloud-downloaded bytes from the previous test.
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(launchSaveFile(), 'utf-8') === 'post-play-save-content';
        } catch {
          return false;
        }
      },
      { timeout: REAP_TIMEOUT, timeoutMsg: 'the play-stub never wrote its post-play content' },
    );

    await browser.waitUntil(
      async () => {
        const uploads = (await mockRequests())
          .slice(before)
          .filter((r) => r.method === 'POST' && r.path.startsWith('/api/saves?'));
        return uploads.length > 0;
      },
      {
        timeout: UPLOAD_AFTER_EXIT_TIMEOUT,
        timeoutMsg: 'no auto-upload POST /api/saves arrived after exit',
      },
    );

    // The auto upload's own toast — the round-4 gap this closes. The wait
    // above returns as soon as the POST reaches the mock, and the event is
    // emitted on the same task right after that POST resolves, so the toast
    // is still inside its 4 s window here.
    await $(testId('toast')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared after the auto upload on exit',
    });
    await expect($(testId('toast'))).toHaveText('SaveSyncLaunch — Uploaded 1 save files.');

    const uploads = (await mockRequests())
      .slice(before)
      .filter((r) => r.method === 'POST' && r.path.startsWith('/api/saves?'));
    expect(uploads).toHaveLength(1);
    expect(uploads[0].query?.rom_id).toBe('602');
    expect(uploads[0].query?.overwrite).toBe('true');
    expect(uploads[0].multipart?.[0].text).toBe('post-play-save-content');

    await closeDetails();
  });

  it('retention: uploading against 4 seeded records in one slot prunes exactly one', async () => {
    await openDetails(603);
    await openSavePanel();

    const before = (await mockRequests()).length;
    await $(testId('cloud-upload')).click();

    await browser.waitUntil(
      async () => {
        const deletes = (await mockRequests())
          .slice(before)
          .filter((r) => r.method === 'POST' && r.path === '/api/saves/delete');
        return deletes.length > 0;
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no POST /api/saves/delete arrived after upload' },
    );

    const after = (await mockRequests()).slice(before);
    const deletes = after.filter((r) => r.method === 'POST' && r.path === '/api/saves/delete');
    expect(deletes).toHaveLength(1);
    expect(deletes[0].bodyJson).toEqual({ saves: [9004] });

    await closeDetails();
  });

  it('block reason: a native-platform game is unsupported for save states', async () => {
    // `details_cloud_mode_supported` (ops/mod.rs) returns `false` for
    // EVERY native (Windows) game's state mode, unconditionally — so
    // Details.svelte never renders the "Manage States" toggle for one
    // (`{#if statePanelInfo?.supported}`, Details.svelte), and
    // CloudPanel.svelte's own doc comment on `nativeStateBlocked` says
    // that combination is "only reachable defensively" through the UI.
    // This calls the real `cloud_panel_info` command directly — the same
    // IPC bridge `api.ts`'s `cloudPanelInfo` uses — to exercise the
    // backend computation a native+state panel would show if it could
    // ever be reached by a click.
    const result = await browser.execute(async () => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Function } })
        .__TAURI_INTERNALS__.invoke;
      return invoke('cloud_panel_info', {
        game: { title: 'Native Block Reason Check', platform: 'Windows', rom_id: null },
        saveType: 'state',
      });
    });

    expect(result).toEqual({
      supported: false,
      block_reason: 'Cloud save management is only available for emulator-based games.',
      scope: 'per_game',
    });
  });
});
