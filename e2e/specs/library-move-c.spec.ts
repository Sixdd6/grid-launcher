import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  expectConfigSwitched,
  expectEmulatorFilesRewritten,
  expectRegistryMoved,
  expectSavesLink,
  MOVED_DIRS,
  reachShell,
  romPath,
  testId,
} from '../helpers/libraryMove.js';

const missingRoot = () => path.join(dataDir(), 'relocate-missing');
const realRoot = () => path.join(dataDir(), 'relocate-real');

/**
 * Stage `library-move`, part C (9b): the library folder cannot be reached
 * at startup.
 *
 * `e2e/seed/library-move-seed.mjs library-move-c` stores
 * `<data>/relocate-missing` as the library (it does not exist) with an
 * installed row and emulator entries under it, while the files themselves
 * are in `<data>/relocate-real` (and the PCSX2 ini and xemu.toml there still
 * name the missing folder). GRID must show the blocking unreachable-library
 * dialog instead of Connect; "Relocate the library…" to the real folder
 * rewrites every stored path, moves no file, and lets the app continue.
 */
describe('library-move (c): relocate an unreachable library', () => {
  it('shows the unreachable-library dialog instead of Connect', async () => {
    await $(testId('library-unreachable')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the unreachable-library dialog never appeared',
    });
    await expect($(testId('library-unreachable-root'))).toHaveText(missingRoot());
    expect(await $(testId('connect-server-url')).isExisting()).toBe(false);
    expect(await $(testId('platform-btn-1')).isExisting()).toBe(false);
  });

  it('relocates the library to the folder where the files are', async () => {
    await $(testId('library-unreachable-path')).setValue(realRoot());
    await $(testId('library-unreachable-relocate')).click();
    await $(testId('library-unreachable')).waitForExist({
      timeout: TRANSITION_TIMEOUT * 2,
      reverse: true,
      timeoutMsg: 'the unreachable-library dialog never went away after Relocate',
    });
  });

  it('points the config and the registry at the chosen folder', () => {
    expectConfigSwitched(missingRoot(), realRoot());
    expectRegistryMoved(realRoot());
  });

  it('rewrites the emulator files and moves no file', () => {
    expectEmulatorFilesRewritten(missingRoot(), realRoot());
    expectSavesLink(realRoot(), 'inis');
    expectSavesLink(realRoot(), 'memcards');
    for (const dir of MOVED_DIRS) expect(existsSync(path.join(realRoot(), dir))).toBe(true);
    expect(readFileSync(romPath(realRoot()), 'utf-8')).toBe('fake snes rom bytes\n');
    expect(existsSync(missingRoot())).toBe(false);
  });

  it('continues to the normal flow with the game still installed', async () => {
    await reachShell();
    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(`${testId('server-view')} ${testId('installed-badge-101')}`).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'rom 101 does not show as installed after the relocate',
    });
  });
});
