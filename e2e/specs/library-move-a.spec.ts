import { existsSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import { dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  argvFile,
  expectConfigSwitched,
  expectEmulatorFilesRewritten,
  expectRegistryMoved,
  expectSavesLink,
  expectTreeMoved,
  journalFile,
  reachShell,
  romPath,
  testId,
  waitForMoveCompleted,
} from '../helpers/libraryMove.js';

const oldRoot = () => path.join(dataDir(), 'library');
const newRoot = () => path.join(dataDir(), 'library-new');

/**
 * Stage `library-move`, part A (9b): Settings › Library › Change… › Move
 * existing files, start to finish.
 *
 * `e2e/seed/library-move-seed.mjs library-move-a` writes a current-layout
 * library at `<data>/library` (an installed rom 101, a stub SNES emulator,
 * PCSX2 with its user-data links into `saves/`, xemu with an `xemu.toml`,
 * and an unrelated `notes.txt`). e2e.sh sets the e2e-only
 * GRID_LAUNCHER_E2E_FORCE_COPY_MOVE for this group, so every folder takes
 * the Copy path (copy, verify, commit, delete the source) although the
 * whole run is on one disk.
 */
describe('library-move (a): move the library to a new folder', () => {
  before(async () => {
    await reachShell();
  });

  it('shows the current folder in Settings › Library', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('settings-nav-library')).click();
    await $(testId('settings-library-pane')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-path-value'))).toHaveText(oldRoot());
  });

  it('previews and starts the move', async () => {
    await $(testId('library-path-change')).click();
    await $(testId('library-change-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('library-new-path-input')).setValue(newRoot());
    await $(testId('library-change-move')).waitForEnabled({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Move existing files never became available for a valid new folder',
    });
    await $(testId('library-change-move')).click();

    // The confirm step lists the three folders once the preview is back.
    await $(testId('library-move-summary')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the move preview never appeared',
    });
    const list = await $(testId('library-move-list')).getText();
    for (const dir of ['games/', 'emulators/', 'saves/']) expect(list).toContain(dir);
    // The forced Copy reached the app: no folder is a same-drive rename.
    expect(list).toContain('Copied and checked');
    expect(list).not.toContain('Same drive');
    await $(testId('library-move-confirm')).waitForEnabled({ timeout: TRANSITION_TIMEOUT });
    await $(testId('library-move-confirm')).click();

    await $(testId('library-change-dialog')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the change dialog never closed after Start move',
    });
    expect(await $(testId('library-change-error')).isExisting()).toBe(false);
  });

  it('finishes the move on the Downloads row', async () => {
    await waitForMoveCompleted();
    expect(existsSync(journalFile())).toBe(false);
  });

  it('moves games, emulators and saves, and leaves the unrelated file', () => {
    expectTreeMoved(oldRoot(), newRoot());
  });

  it('switches the config to the new folder', () => {
    expectConfigSwitched(oldRoot(), newRoot());
  });

  it('rewrites the registry row', () => {
    expectRegistryMoved(newRoot());
  });

  it('rewrites the absolute paths in PCSX2.ini and xemu.toml', () => {
    expectEmulatorFilesRewritten(oldRoot(), newRoot());
  });

  it('keeps the user-data links resolving into the new saves folder', () => {
    expectSavesLink(newRoot(), 'inis');
    expectSavesLink(newRoot(), 'memcards');
  });

  it('shows the new folder in Settings › Library', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-library-pane')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-path-value'))).toHaveText(newRoot(), {
      wait: TRANSITION_TIMEOUT,
    });
    await expect($(testId('library-path-change'))).toBeEnabled();
  });

  it('launches the moved game from the new folder', async () => {
    rmSync(argvFile(), { force: true });
    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 101',
    });
    await $(testId('details-play')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-play never appeared for the moved game',
    });
    await $(testId('details-play')).click();

    await browser.waitUntil(() => existsSync(argvFile()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the stub emulator never wrote its argv file',
    });
    await browser.waitUntil(() => readFileSync(argvFile(), 'utf-8').endsWith('\n'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the stub emulator never finished writing its argv file',
    });
    expect(readFileSync(argvFile(), 'utf-8').trim().split('\n')).toEqual([romPath(newRoot())]);
  });
});
