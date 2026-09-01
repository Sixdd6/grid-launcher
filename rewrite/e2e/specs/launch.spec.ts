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
 * Stage `launch`: emulated-launch flows against rom 101 ("Super Mario
 * World" on "Super Nintendo Entertainment System"), pre-seeded as already
 * installed by `rewrite/e2e/seed/launch-seed.mjs` (run by
 * `rewrite/scripts/e2e.sh`'s `seed_script_for_group` before the app starts
 * — Ruling A in task-7-brief.md). That seed also writes config.toml with
 * three emulator entries:
 *
 * - "LongRunner": a stub that records its argv then sleeps — the default
 *   for the platform at app start.
 * - "InstantExit": a stub that exits immediately (code 3).
 * - "RetroArch": a stub whose basename ("retroarch") matches the
 *   `emulator-autoprofiles.json` "RetroArch (Multi-System)" profile
 *   (`all_platforms: true`), with args `-L "%core%" "%rom%"` and no core
 *   mapped in `retroarch_cores`.
 *
 * Every flow below switches which of these is the platform's default, or
 * edits one's path, through the Emulators UI rather than by seeding a
 * second stage — simpler, and it doubles as coverage of the defaults-select
 * and edit flows against a real launch, not just the config file (see
 * emulators.spec.ts for those in isolation). Each mutation is followed by a
 * config.toml read-back (`waitForConfigLine`) before the next Play, since
 * `resolve_launch` reads the file fresh on every launch and the UI gives no
 * other signal that an update actually reached disk.
 */
describe('launch', () => {
  const PLATFORM = 'Super Nintendo Entertainment System';
  const romPath = () =>
    path.join(dataDir(), 'library', PLATFORM, 'Super Mario World', 'game.sfc');
  const longRunnerArgv = () => path.join(dataDir(), 'stubs', 'long-runner.args');

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

  async function openDetails() {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 101',
    });
  }

  async function openEmulators() {
    await $(testId('emulators-open')).click();
    await $(testId('emulators-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators panel never opened',
    });
  }

  async function closeEmulators() {
    await $(testId('emulators-close')).click();
    await $(testId('emulators-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the emulators panel never closed',
    });
  }

  /**
   * Sets a native `<select>`'s value and dispatches `change` directly. A
   * WebDriver-protocol click on the underlying `<option>` (what
   * `selectByAttribute` does) does not reliably fire a `change` event on
   * this embedded WebKitGTK driver — confirmed by config.toml never picking
   * up the new default when this test used `selectByAttribute` instead.
   */
  async function selectValue(testIdName: string, value: string) {
    await browser.execute(
      (selector, val) => {
        const el = document.querySelector(selector) as HTMLSelectElement | null;
        if (!el) throw new Error(`no element matched ${selector}`);
        el.value = val;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      },
      testId(testIdName),
      value,
    );
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
  });

  it('plays the pre-seeded installed game, passing the rom path to the emulator, then stops it', async () => {
    await openDetails();
    await $(testId('details-play')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-play never appeared for the pre-seeded installed game',
    });

    await $(testId('details-play')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-playing-chip never appeared after Play',
    });

    await browser.waitUntil(() => existsSync(longRunnerArgv()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the long-runner stub never wrote its argv file',
    });
    const argv = readFileSync(longRunnerArgv(), 'utf-8').trim().split('\n');
    expect(argv).toEqual([romPath()]);

    await $(testId('details-stop')).click();
    await $(testId('details-playing-chip')).waitForExist({
      timeout: REAP_TIMEOUT,
      reverse: true,
      timeoutMsg: 'details-playing-chip never cleared after Stop within the reaper window',
    });
    await $(testId('details-play')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Play button never came back after stopping',
    });
    await $(testId('details-close')).click();
  });

  it('shows "exited immediately" after switching the default to the instant-exit stub', async () => {
    await openEmulators();
    await selectValue('default-select-1', 'InstantExit');
    await waitForConfigLine(`"${PLATFORM}" = "InstantExit"`);
    await closeEmulators();

    await openDetails();
    await $(testId('details-play')).click();
    await $(testId('details-warning')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-warning never appeared for the instant-exit stub',
    });
    await expect($(testId('details-warning'))).toHaveText('exited immediately', { containing: true });
    await $(testId('details-warning-dismiss')).click();
    await $(testId('details-close')).click();
  });

  it('shows the verbatim "Emulator executable not found:" error when the default\'s path is broken', async () => {
    await openEmulators();
    await $(testId('emulator-edit-instantexit')).click();
    await $(testId('emu-form-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('emu-form-path')).setValue('/nonexistent/no-such-emulator');
    await $(testId('emu-form-save')).click();
    await waitForConfigLine('/nonexistent/no-such-emulator');
    await closeEmulators();

    await openDetails();
    await $(testId('details-play')).click();
    await $(testId('details-error')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-error never appeared for the broken emulator path',
    });
    await expect($(testId('details-error'))).toHaveText('Emulator executable not found:', {
      containing: true,
    });
    await $(testId('details-close')).click();
  });

  it('shows the verbatim "No RetroArch core is configured" error for an unmapped RetroArch default', async () => {
    await openEmulators();
    await selectValue('default-select-1', 'RetroArch');
    await waitForConfigLine(`"${PLATFORM}" = "RetroArch"`);
    await closeEmulators();

    await openDetails();
    await $(testId('details-play')).click();
    await $(testId('details-error')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-error never appeared for the unmapped RetroArch default',
    });
    // The template layer's own validation error ("No RetroArch core is
    // configured...") is wrapped by prepare_emulator_launch (spawn.rs) as
    // "Invalid launch arguments: <e>" — pinned by spawn.rs's
    // an_argument_failure_is_wrapped unit test. Assert the full wrapped
    // string verbatim, not just the inner message.
    await expect($(testId('details-error'))).toHaveText(
      'Invalid launch arguments: No RetroArch core is configured for this platform. ' +
        'Set one in Emulators > Defaults.',
    );
  });
});
