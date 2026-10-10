import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  argvFile,
  PLATFORM,
  reachShell,
  registryValue,
  testId,
  TITLE,
} from '../helpers/libraryMove.js';

const root = () => path.join(dataDir(), 'reconcile');
const elsewhere = () => path.join(dataDir(), 'reconcile-elsewhere');
/** Where the seed installed the game (layout v2). */
const recordedDir = () => path.join(root(), 'games', PLATFORM, TITLE);
/** The legacy platform folder under the same root: a place 9c searches. */
const movedDir = () => path.join(root(), PLATFORM, TITLE);
/** A copy outside the library, found only through Locate…. */
const copyDir = () => path.join(elsewhere(), TITLE);

/** Leaves the Library and comes back: every activation re-reads the list. */
async function refreshLibrary(): Promise<void> {
  await $(testId('nav-server')).click();
  await $(testId('server-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  await $(testId('nav-library')).click();
  await $(testId('library-card-101')).waitForExist({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'library-card-101 never appeared',
  });
}

/** Waits for the stub emulator's argv file and answers its lines. */
async function launchedWith(): Promise<string[]> {
  await browser.waitUntil(() => existsSync(argvFile()), {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the stub emulator never wrote its argv file',
  });
  await browser.waitUntil(() => readFileSync(argvFile(), 'utf-8').endsWith('\n'), {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the stub emulator never finished writing its argv file',
  });
  return readFileSync(argvFile(), 'utf-8').trim().split('\n');
}

/**
 * Stage `library-move`, part D (9c): moved-file detection.
 *
 * `e2e/seed/library-move-seed.mjs library-move-d` writes a current-layout
 * library at `<data>/reconcile` with rom 101 installed at
 * `games/<SNES>/Super Mario World/` and a stub SNES emulator that writes its
 * argv. The spec moves and deletes the game folder while the app runs:
 *
 * 1. moved to the legacy `<root>/<SNES>/` folder → the next Library refresh
 *    re-points the row there, and the game plays from it;
 * 2. deleted → the card shows the Missing badge; Details shows "Files not
 *    found" with Play disabled; Locate… refuses a folder without the game
 *    and accepts `<data>/reconcile-elsewhere`, which holds a copy outside
 *    the library; the game then plays from the copy.
 */
describe('library-move (d): moved-file detection', () => {
  before(async () => {
    await reachShell();
  });

  it('re-points a game moved inside the library on the next refresh', async () => {
    mkdirSync(path.dirname(movedDir()), { recursive: true });
    renameSync(recordedDir(), movedDir());
    await refreshLibrary();
    await browser.waitUntil(() => registryValue('extracted_dir') === movedDir(), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the row was not re-pointed to ${movedDir()} (it reads ${registryValue('extracted_dir')})`,
    });
    expect(registryValue('extracted_path')).toBe(path.join(movedDir(), 'game.sfc'));
    expect(await $(testId('library-card-missing-101')).isExisting()).toBe(false);
  });

  it('plays the moved game from its new folder', async () => {
    rmSync(argvFile(), { force: true });
    await $(testId('card-primary-101')).click();
    expect(await launchedWith()).toEqual([path.join(movedDir(), 'game.sfc')]);
  });

  it('shows the Missing badge once the folder is gone', async () => {
    rmSync(movedDir(), { recursive: true, force: true });
    mkdirSync(copyDir(), { recursive: true });
    writeFileSync(path.join(copyDir(), 'game.sfc'), 'fake snes rom bytes\n');
    await refreshLibrary();
    await $(testId('library-card-missing-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Missing badge never appeared for the deleted game',
    });
    // Missing is computed, never stored: the row keeps its last paths.
    expect(registryValue('extracted_dir')).toBe(movedDir());
  });

  it('shows Files not found in Details with Play disabled', async () => {
    // A missing card's primary button opens Details instead of launching.
    await $(testId('card-primary-101')).click();
    await $(testId('details-missing')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Details never showed the missing block',
    });
    await expect($(testId('details-missing'))).toHaveText(expect.stringContaining('Files not found'));
    await $(testId('details-play')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-play'))).toBeDisabled();
    await expect($(testId('details-uninstall'))).toBeDisabled();
    await expect($(testId('details-missing-remove'))).toBeEnabled();
  });

  it('refuses a folder that does not hold the game', async () => {
    const empty = path.join(elsewhere(), 'empty');
    mkdirSync(empty, { recursive: true });
    await $(testId('details-locate-path')).setValue(empty);
    await $(testId('details-locate-submit')).click();
    await $(testId('locate-error')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Locate… never refused a folder without the game',
    });
    await expect($(testId('locate-error'))).toHaveText(expect.stringContaining(`does not contain “${TITLE}”`));
    expect(registryValue('extracted_dir')).toBe(movedDir());
  });

  it('locates the copy outside the library and plays it', async () => {
    await $(testId('details-locate-path')).setValue(elsewhere());
    await $(testId('details-locate-submit')).click();
    await $(testId('details-missing')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the missing block never went away after Locate…',
    });
    expect(registryValue('extracted_dir')).toBe(copyDir());
    expect(registryValue('extracted_path')).toBe(path.join(copyDir(), 'game.sfc'));

    rmSync(argvFile(), { force: true });
    // The first launch's stub may still run: Details shows Stop until it ends.
    await $(testId('details-play')).waitForEnabled({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-play never became enabled after Locate…',
    });
    await $(testId('details-play')).click();
    expect(await launchedWith()).toEqual([path.join(copyDir(), 'game.sfc')]);
  });
});
